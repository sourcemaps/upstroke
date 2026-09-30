# `src/engine/topology/identity.rs`

Extended notes for [`src/engine/topology/identity.rs`](../../../../src/engine/topology/identity.rs).

The code is the authority for what it does; this file is the whole of its prose, moved out of
the source verbatim. Each section is headed by the line of code the comment sat above, spelled
as it is in the source, so the heading is the grep string that finds the code.

## Module

Process-lifetime identity: invocations, slot pairs, provisional reservations.

`decisions.admission_and_leases.invocation_identity` defines the value;
`src/runner/invocation.rs` defines the type. Neither allocates one — that
module says so in as many words: *"PR4 owns the type and its properties.
**PR7 assigns them**. No ledger, no broker and no allocation policy lives
here."* This is the ledger, the assignment, and the policy.

Three concerns live together because they share one lifetime and one
failure mode. An [`InvocationId`], a slot pair and a provisional
reservation are all **process-local**: `crash_reconstruction` requires that
"provisional reservations, slot table, invocation ledger, and the
coordinator's own lock holds are empty at process start", and a resume
rebuilds none of them. A ledger that survived a process would be a claim
about a dead coordinator's state, which is precisely what the recovery order
exists to avoid making.

### The ledgers the broker composes, and the one place nothing waits

At `max_parallel = 1` the packet asked for **assertions**:
`state_resource_ownership_matrix` recorded R3 as "assertion only" and the
pipeline entitlement as "sequential assertion", and this file held a
`SlotAssertion` that refused a second concurrent slotted invocation outright.
PR11's broker (`src/engine/topology/permits.rs`) is where waiting arrives,
and it waits **here**, in these ledgers: [`SlotTable`] queues a request whose
pair is not free, in arrival order, with the head reserving what it waits
for, and grants it when a release makes room; [`Reservations`] holds several
provisional reservations at once, one per selected task, taken only when the
fold-derived count plus the outstanding ones permits; [`InvocationLedger`] is
R4 with R3's table inside it, so releasing a pair *is* settling the
registration that holds it and cannot happen twice.

The two types keep their names and `new()` because `EmitContext` in the
frozen `recover.rs` (`:1392-1399`) carries `&mut Reservations` and
`&mut InvocationLedger`; they were evolved in place rather than replaced.

What still does not wait is the **synchronous substrate**. At width 1 the
coordinator runs every invocation itself, on its own thread, and INV-18 says
"The coordinator never blocks on an entitlement, provisional reservation, or
slot". So the substrate registers through
[`InvocationLedger::register_at_once`]: a pair it cannot be granted at once is
a hold some caller leaked, and the request is withdrawn — registered, queued,
cancelled, the head recomputed — and refused with that sentence. That is the
old refusal, spoken in the broker's vocabulary rather than an assertion's.

## `pub struct AttemptIdentities {`

---------------------------------------------------------------------------
Assignment
---------------------------------------------------------------------------

## `pub struct AttemptIdentities {`

Every invocation identity of one attempt.

A value rather than four free functions, because the three coordinates that
must not vary within an attempt — key, generation, attempt number — are then
fixed once at the top of the attempt and cannot be mistyped at the fourth
call site. `decisions.admission_and_leases.invocation_identity`'s first
form is exactly this tuple.

**A retry is a new attempt number, so it is a new `AttemptIdentities`.**
INV-20: "every Runner process carries a unique typed `InvocationId` that
changes with every attempt". Reusing this value across a retry would give
the retry's worker the identity of the attempt that was retained, and a
completion arriving late from the first would then apply to the second.

## `impl AttemptIdentities` › `pub const fn new(key: TaskKey, generation: GenerationId, attempt: AttemptNumber) -> Self {`

The identities of `(key, generation, attempt)`.

## `impl AttemptIdentities` › `pub const fn worker(&self) -> InvocationId {`

The worker process.

## `impl AttemptIdentities` › `pub const fn gate(&self, gate: u32, ordinal: u32) -> InvocationId {`

Gate `gate` of this attempt's gate list, on its `ordinal`-th run.

Two numbers because they mean different things and the packet keeps
them apart: `gate` is *which gate*, `ordinal` is *which run of it*. A
gate re-dispatched inside one attempt is a new identity rather than a
reused one, which is what makes a stale completion from the first run
discardable.

## `impl AttemptIdentities` › `pub const fn review_pass(&self, pass: u32, ordinal: u32) -> InvocationId {`

Review pass `pass`, on its `ordinal`-th run.

## `impl AttemptIdentities` › `pub const fn review_reask(&self, reask: u32, ordinal: u32) -> InvocationId {`

Re-ask `reask` of a review pass, on its `ordinal`-th run.

## `pub struct SequenceIdentities {`

Every invocation identity of one integration transaction.

The packet's second form, "`(sequence, role, ordinal)` with role in
{gate(n), review_pass(n), review_reask(n)}" — **no worker**. A sequence
integrates candidates other processes produced, so there is no worker of a
sequence to identify, and [`SequenceRole`] makes that a compile error rather
than a refusal.

Present in this slice because the identities are PR7's to assign and the
type has to exist for `checkpoint_refusals` to refuse an integration
*before any append*. The transaction itself is PR8's.

## `impl SequenceIdentities` › `pub const fn new(sequence: SequenceId) -> Self {`

The identities of `sequence`.

## `impl SequenceIdentities` › `pub const fn gate(&self, gate: u32, ordinal: u32) -> InvocationId {`

Gate `gate` of this transaction, on its `ordinal`-th run.

## `impl SequenceIdentities` › `pub const fn review_pass(&self, pass: u32, ordinal: u32) -> InvocationId {`

Review pass `pass`, on its `ordinal`-th run.

## `impl SequenceIdentities` › `pub const fn review_reask(&self, reask: u32, ordinal: u32) -> InvocationId {`

Re-ask `reask`, on its `ordinal`-th run.

## `pub struct PreflightIdentities;`

The `RunnerPreflight`'s identities: one shell probe, one probe per agent.

INV-23: "one non-slotted shell probe (the recorded shell executing `exit 0`)
and one slotted probe per recorded agent, each a registered invocation
through the run's Runner". The asymmetry is the whole point of keeping them
apart here — see [`is_slotted`].

These identities **repeat across incarnations** by construction: a probe is
`(probe, target, ordinal)` and carries no run or epoch. That is deliberate
and is why a container name additionally carries the coordinator incarnation
id — without it a resuming incarnation's probe container would collide with,
and overwrite the ownership evidence of, the dead incarnation's.

## `impl PreflightIdentities` › `pub fn shell(ordinal: u32) -> Result<InvocationId, UpstrokeError> {`

The shell probe. Non-slotted.

### Errors

Never in practice — [`InvocationId::probe`] refuses only on an agent id
this target does not carry — but the fallibility is [`ProbeTarget`]'s
and is not worth a second, unfalsifiable, constructor to hide.

## `impl PreflightIdentities` › `pub fn agent(agent: &str, ordinal: u32) -> Result<InvocationId, UpstrokeError> {`

The probe of one recorded agent. Slotted.

### Errors

[`UpstrokeError`] when `agent` is not a name an invocation id can carry
— outside `[0-9A-Za-z_-]`, or too long. A probe identity is a path and
a container-name component, so the refusal is a containment refusal.

## `pub fn is_slotted(invocation: &InvocationId) -> bool {`

---------------------------------------------------------------------------
Slot pairs — R3's table, queued and granted
---------------------------------------------------------------------------

## `pub fn is_slotted(invocation: &InvocationId) -> bool {`

Whether `invocation`'s process takes an atomic `{agent, pool?}` slot pair.

`permits.agent_pool_slots` lists the slotted roles and then excludes two by
name: "**gate invocations and the shell probe acquire no slot**". Both
exclusions are recoverable from the identity alone — a gate is
`AttemptRole::Gate`/`SequenceRole::Gate`, the shell probe is
`ProbeTarget::Shell` — so this is a total function of the id rather than a
second field a caller could set wrongly.

[`crate::runner::ExecutionRole::is_slotted`] states the same rule over the
request's role. The two agree by construction because both read the packet
sentence, and `a_gate_and_the_shell_probe_are_refused_a_slot_pair` pins
this side of it; `src/runner/**` is frozen, so they cannot be unified here.

## `pub struct SlotPair {`

The atomic pair a slotted invocation holds.

## `pub struct SlotPair` › `pub agent: String,`

The agent whose per-agent slot this is.

## `pub struct SlotPair` › `pub pool: Option<String>,`

The pool, when the agent is in one. An agent without a pool takes its agent
slot only (acceptance item 3, "agent without pool uses the agent slot only").

## `impl fmt::Display for SlotPair {`

How a refusal names the pair a request waited for.

## `pub struct SlotLimits {`

R3's two limits: `max_per_agent` and `max_per_pool`.

`permits.agent_pool_slots`: "per-agent slots (max_per_agent, default
max_parallel) and per-pool slots keyed by pool name (max_per_pool, default
max_parallel); process-lifetime ephemeral scheduler state". Neither is
recorded in `run_started(4)` — `TopologyLimits` holds `max_parallel`,
`max_defers` and `max_merge_repairs` and nothing else, and PR11 adds no
durable field — so they are the command's configuration (`[engine]
max_per_agent`/`max_per_pool`, parsed and bounded below by
`src/config/parse.rs`), defaulting to the recorded `max_parallel`. Record
§3 R-K states where each comes from.

Non-zero by type: a limit of zero would hold every request for that agent or
pool pending forever, which `permits.deadlock_freedom` does not allow, and
the configuration refuses it too.

## `impl SlotLimits` › `pub const WIDTH_ONE: Self = Self {`

One per agent and one per pool: the defaults at `max_parallel = 1`.

## `impl SlotLimits` › `pub fn new(per_agent: u32, per_pool: u32) -> Result<Self, UpstrokeError> {`

The configured limits.

### Errors

[`UpstrokeError::Refused`] when either is zero.

## `impl SlotLimits` › `pub fn defaulted(max_parallel: u32) -> Self {`

Both limits at the run's recorded `max_parallel`, the packet's default.

A `max_parallel` of zero is read as one. The fold refuses a `run_started`
recording zero (`src/topology/fold/start.rs`), so no started run reaches
the floor; it exists so the function is total.

## `pub struct SlotTable {`

R3: the held pairs, and the requests waiting for theirs.

Held counts are **derived** from the holders every time they are read, never
stored, so no count can drift from the pairs it counts and none can go
negative: releasing a pair removes a holder, and a holder that is not there
cannot be removed twice. The waiting requests are a queue in arrival order.

### The grant rule: FIFO with head reservation

The head of the queue — the oldest waiting request — **reserves every slot it
waits for**: one slot of its agent and, when it has a pool, one slot of its
pool. A request is granted when:

- it is the head, and its pair fits the free capacity
  (`held + 1 <= limit` for its agent and for its pool); or
- it is behind the head, and its pair fits the capacity **left after the
  head's reservation** (`held + 1 + 1 <= limit` for a resource the head also
  needs, `held + 1 <= limit` for any other).

So a later request is granted past a waiting head — backfilled — only when
it takes nothing the head waits for, and the head is granted at the first
release that makes its pair fit. Strict head-of-line FIFO was the packet's
rejected alternative ("blocks disjoint work; head reservation adopted",
`alternatives[74]`); this is the adopted rule, stated as the reading of the
packet's "FIFO with head reservation" in record §3 R-L.

Why the head cannot starve: while it waits, no grant can take a slot it
reserved, so every holder of a resource it is blocked on was granted before
it became the head; those are running processes that wait for nothing
(`permits.deadlock_freedom`) and end, and the first release that frees its
last blocking slot grants it. Every later request becomes the head in turn.
`seeded_adversarial_orders_grant_every_request_in_order_and_exceed_no_limit`
(in `permits.rs`) checks exactly that invariant after every operation.

## `impl SlotTable` › `pub fn new() -> Self {`

An empty table at [`SlotLimits::WIDTH_ONE`], which is what process start
requires of the sequential substrate.

## `impl SlotTable` › `pub const fn with_limits(limits: SlotLimits) -> Self {`

An empty table at `limits`.

## `impl SlotTable` › `fn enqueue(&mut self, invocation: &InvocationId, pair: SlotPair) -> Vec<InvocationId> {`

Queue a request at the tail and grant whatever now fits. Only the ledger
calls this, when it registers a slotted invocation, so a request exists
exactly when its registration is pending.

## `impl SlotTable` › `fn release(&mut self, invocation: &InvocationId) -> Option<Vec<InvocationId>> {`

Release `invocation`'s pair and grant whatever the release made room for.

`None` when it holds no pair: an unslotted registration, whose settlement
releases nothing.

## `impl SlotTable` › `fn withdraw(&mut self, invocation: &InvocationId) -> Option<Vec<InvocationId>> {`

Remove a waiting request. When it was the head, the next request becomes the
head and the reservation moves to it — `permits.protocol`'s "a pending
slotted request is removed (head reservation recomputed)" — which can grant
requests the old head's reservation had kept waiting.

## `impl SlotTable` › `fn withdraw_all(&mut self) -> Vec<InvocationId> {`

Remove every waiting request **without granting anything**.

Shutdown and the append-error protocol cancel what is pending; withdrawing
them one at a time would grant the second when the first left the head.

## `impl SlotTable` › `fn pump(&mut self) -> Vec<InvocationId> {`

The grant rule above, applied once: grant heads while they fit, then
backfill the rest in arrival order against the capacity the (new) head
leaves. Returns the grants in the order they were made.

One pass is enough: a pass only adds holders, so a request that did not fit
earlier in it cannot fit later in it.

## `impl SlotTable` › `fn fits(&self, pair: &SlotPair, reserved: Option<&SlotPair>) -> bool {`

Whether `pair` fits, leaving room for `reserved` — the head's pair — on every
resource the two share.

## `impl SlotTable` › `pub fn head(&self) -> Option<&InvocationId> {`

The oldest waiting request, which holds the reservation.

## `impl SlotTable` › `pub fn reserved(&self) -> Option<&SlotPair> {`

What the head reserves: its pair.

## `impl SlotTable` › `pub fn blocking(&self, pair: &SlotPair) -> Vec<&InvocationId> {`

The holders of the agent or the pool `pair` needs, so a refusal can name the
hold that kept it waiting.

## `impl SlotTable` › `pub fn is_empty(&self) -> bool {`

Nothing held and nothing waiting: the process-start and process-end state.

## `impl SlotTable` › `pub fn balances(&self) -> bool {`

Every grant released, and the table empty.

## `pub enum ReservationKind {`

---------------------------------------------------------------------------
Provisional reservations
---------------------------------------------------------------------------

## `pub enum ReservationKind {`

What a provisional reservation bridges to.

`permits.provisional_reservations`: "process-lifetime bridge between a
selection decision and its first append: dispatch selection reserves
{pipeline} until `task_dispatched`; retry selection reserves {pipeline}
until `attempt_started(retry)`; integration selection reserves
{pipeline, merge} until `merge_prepared(fast)`, `merge_verification_started`
or `merge_rejected(conflict)`."

## `pub enum ReservationKind` › `Dispatch,`

A fresh dispatch, converted at `task_dispatched`.

## `pub enum ReservationKind` › `Retry,`

A same-generation retry, converted at `attempt_started(retry)`.

## `pub enum ReservationKind` › `Integration,`

An integration transaction. Holds `{pipeline, merge}`, not `{pipeline}`, as
**one** reservation: the pair is taken, converted and cancelled together,
which is what makes the fast path's conversion at `merge_prepared(fast)`
atomic.

## `impl ReservationKind` › `pub const fn entitlements(self) -> u32 {`

How many entitlements this reservation holds.

Dispatch and retry hold `{pipeline}`; integration holds
`{pipeline, merge}`. The count is what has to balance.

## `impl ReservationKind` › `pub const fn name(self) -> &'static str {`

The name the refusal messages use.

## `impl ReservationKind` › `pub const fn holds_merge(self) -> bool {`

Whether this reservation holds the merge entitlement.

## `pub struct Reservations {`

R13: the process-local provisional-reservation ledger.

"crash reset: none exist at process start", "process-local ledger balances
at process end": `new` is empty, and `balances` is the process-end check.

**Several at once, each bound to its selection.** At width above one several
reservations are outstanding together. Each is bound to its **selection
identity**, the task the selection chose and the kind of admission —
`(TaskKey, ReservationKind)`, at most one per task (record §3 R-M): the
packet's matrix gives "one per selected task or candidate", a queued
candidate is its task's (the queue holds at most one per task), and the
frozen `IntegrationJournal::converted(key)` converts by the key alone.

**Settled exactly once, duplicates counted.** A conversion or cancellation
naming a reservation that was taken and is no longer outstanding is a
duplicate or a stale operation: ignored, counted, releasing nothing. One
naming a reservation that was never taken, or naming the task's outstanding
reservation under another kind, is refused — the first is a caller that
settles what it never reserved, the second the shape that would count an
entitlement against the wrong admission.

Cancellation is not an error path — it is one of four ordinary outcomes:
"cancellation on any pre-append failure, run end, shutdown, or a poisoned
fold".

## `impl Reservations` › `pub fn new() -> Self {`

An empty ledger, which is what process start requires.

## `impl Reservations` › `pub fn take(&mut self, key: TaskKey, kind: ReservationKind) -> Result<(), UpstrokeError> {`

The sequential substrate's form: reserve for `key`, **asserting** that no
reservation is outstanding at all. `permits.provisional_reservations` ends
"the sequential substrate asserts at most one"; the fixtures that drive the
frozen merge and recovery modules at width 1 take their reservations this
way. The broker's own form is [`Self::reserve`].

### Errors

[`UpstrokeError::Refused`] when any reservation is outstanding.

## `impl Reservations` › `pub fn reserve(`

The broker's form: reserve for `key` when the fold-derived count plus the
outstanding reservations permits, and otherwise refuse — **never wait**.

`permits.deadlock_freedom`: "provisional reservations are taken only when
the derived count permits and are converted or cancelled without waiting".
`derived` is [`crate::engine::topology::select::Entitlements`], read from the
fold by the caller at the moment it selects: the pipeline count plus the
outstanding reservations must stay within `max_parallel`, and an integration
reservation needs the one merge entitlement free of the fold's transaction
and of every outstanding integration reservation.

### Errors

[`UpstrokeError::Refused`] when `key` already holds a reservation (a second
for one selected task), when the fold is poisoned, or when the count does not
permit it. Nothing is taken on any of them.

## `impl Reservations` › `pub fn convert(&mut self, key: TaskKey, kind: ReservationKind) -> Result<(), UpstrokeError> {`

Convert `key`'s reservation at its first append. A duplicate is counted, not
refused.

### Errors

As `settle`: a reservation never taken, or one outstanding under another
kind.

## `impl Reservations` › `pub fn cancel(&mut self, key: TaskKey, kind: ReservationKind) -> Result<(), UpstrokeError> {`

Cancel it: a pre-append failure. A duplicate is counted, not refused.

### Errors

As [`Self::convert`].

## `impl Reservations` › `pub fn cancel_all(&mut self) -> usize {`

Cancel every outstanding reservation — run end, shutdown, a poisoned fold —
returning how many there were.

## `impl Reservations` › `pub fn cancel_any(&mut self) -> bool {`

Cancel whatever is held without naming it, reporting whether anything was.

The append-error protocol's shape: the fold is poisoned and the coordinator
is ending, so it cancels what it holds rather than asserting what that is.
Since PR11 that is every outstanding reservation, not the one the sequential
substrate held.

## `impl Reservations` › `pub const fn peak(&self) -> usize {`

The most reservations ever outstanding at once in this ledger, updated as each
is taken. The coordinator converts every provisional reservation at its first
append, on its own thread, before it selects again, so at any width the peak is
one (memo §5; the coordinator's ST-04 witness reads it).

## `impl Reservations` › `fn settle(`

The one place a reservation leaves `held`: converted or cancelled once, a
settled one counted as a duplicate, anything else refused.

## `impl Reservations` › `pub fn entitlements_held(&self) -> u32 {`

The entitlements the outstanding reservations account for, zero when none is.

## `impl Reservations` › `pub fn pipeline_outstanding(&self) -> usize {`

How many pipeline entitlements the outstanding reservations hold: every
kind holds one.

## `impl Reservations` › `pub fn merge_outstanding(&self) -> usize {`

How many merge entitlements they hold: one per integration reservation.

## `impl Reservations` › `pub const fn cancelled(&self) -> u32 {`

How many reservations were cancelled, so a test can tell a reservation
that converted from one that was cancelled: both leave nothing held.

## `impl Reservations` › `pub const fn duplicates(&self) -> u32 {`

How many duplicate or stale conversions and cancellations were ignored.

## `impl Reservations` › `pub fn balances(&self) -> bool {`

Whether every reservation taken was converted or cancelled exactly once, and
none is outstanding.

## `pub enum Admission {`

---------------------------------------------------------------------------
The invocation ledger
---------------------------------------------------------------------------

## `pub enum Admission {`

What a registration came to: runnable at once without slots (a gate, the
shell probe), granted its pair, or waiting for it.

## `enum Registration {`

What an invocation's registration is currently: waiting for its pair,
running, or settled one of two ways.

## `pub struct InvocationLedger {`

R4 — every Runner process registered exactly once and settled exactly
once — with R3's [`SlotTable`] inside it.

`permits.protocol`: "register(invocation_id, slots: None | Some{agent,
pool?}) -> if slotted, wait for the atomic pair grant (FIFO with head
reservation) else immediately runnable -> … -> complete(invocation_id)
releasing any slots -> or cancel(invocation_id): a pending slotted request
is removed (head reservation recomputed); a granted or non-slotted running
invocation is cancelled after the Runner terminated its process or
container; duplicate complete/cancel ignored and counted".

The table is inside the ledger because a pair belongs to a registration: it
is requested when the registration is made, and released when the
registration settles. Settling is the only transition that releases, and a
registration settles once, so "nothing released twice" (ST-02, ST-05) is a
property of the type. "Released only after the Runner reports it terminated"
is the caller's obligation, and every caller here settles after its Runner
call returned.

A duplicate settlement is a counter, not an error — INV-20 asks for "discard
with a non-durable warning", not a refusal.

## `impl InvocationLedger` › `pub fn new() -> Self {`

An empty ledger at [`SlotLimits::WIDTH_ONE`], which is what process start
requires.

## `impl InvocationLedger` › `pub fn with_limits(limits: SlotLimits) -> Self {`

An empty ledger at `limits`.

## `impl InvocationLedger` › `pub fn register(&mut self, invocation: &InvocationId) -> Result<(), UpstrokeError> {`

Register an invocation that takes no slot — a gate or the shell probe — as
running.

### Errors

[`UpstrokeError::Refused`] when `invocation` is slotted (it registers with
its pair), or already registered (aliasing, ST-04).

## `impl InvocationLedger` › `pub fn register_slotted(`

Register a slotted invocation with its pair: granted at once when the pair
fits the grant rule, queued otherwise.

`standing` is the acquisition order made a precondition
(`permits.deadlock_freedom`: "pipeline -> merge -> {agent, pool}"): read from
the fold by [`crate::engine::topology::select::Standing::of`], it admits a
slotted request only from an attempt the fold has in flight, from the
integration verification it has started, or from a pre-flight probe. A slot
is therefore never requested by a pipeline that does not already hold its
entitlements.

### Errors

[`UpstrokeError::Refused`] when `invocation` takes no slot, when `standing`
does not admit it, or when it is already registered. Nothing is registered
on any of them.

## `impl InvocationLedger` › `pub fn register_at_once(`

The synchronous substrate's registration: nothing waits for a pair there.

A slotted request that is not granted at once is withdrawn — its
registration cancelled, the head recomputed — and refused with INV-18's
sentence and the holds it would have waited behind. See "The ledgers the
broker composes" above.

The grants a withdrawal can make are not reported: a caller that uses this
form never leaves a request waiting, so behind a withdrawn request there is
nothing to grant. A ledger shared with callers that do wait is the broker's
`register`, whose grants come back from each settlement.

### Errors

As [`Self::register`] and [`Self::register_slotted`], and
[`UpstrokeError::Refused`] when the pair is not grantable at once.

## `impl InvocationLedger` › `pub fn complete(`

Settle `invocation` as completed, releasing its pair, and return the waiting
requests the release granted. A duplicate is counted, not refused.

### Errors

[`UpstrokeError::Refused`] when `invocation` was never registered, or is
still waiting for its pair: no process of it ran, and `permits.protocol`
removes a pending request by cancelling it.

## `impl InvocationLedger` › `pub fn cancel(`

Settle `invocation` as cancelled — withdrawing it when it was waiting,
releasing its pair when it held one — and return what that granted. A
duplicate is counted, not refused.

### Errors

[`UpstrokeError::Refused`] when `invocation` was never registered.

## `impl InvocationLedger` › `pub fn cancel_all_running(&mut self) -> usize {`

Cancel every registration still in flight, returning how many.

Waiting requests are withdrawn first and all together, so no pending one is
granted on the way out; then every running one is cancelled and its pair
released. The append-error protocol's "in-flight invocations are cancelled
through the Runner" — this is the ledger half of that; the Runner half is
the caller's.

## `impl InvocationLedger` › `pub fn withdraw_pending(&mut self) -> usize {`

Cancel every waiting request, granting nothing — shutdown's "pending
requests cancelled". Running invocations are left for the caller to cancel
once the Runner reports them terminated.

## `impl InvocationLedger` › `pub fn settled(&self, invocation: &InvocationId) -> bool {`

Whether `invocation` has already been completed or cancelled here. The
coordinator reads it before it hands a pipeline's end of an invocation to the
ledger: an end for an invocation the pipeline is not running is discarded
unless the ledger already settled it, in which case the ledger's own duplicate
refusal counts it.

## `impl InvocationLedger` › `pub const fn duplicates(&self) -> u32 {`

How many duplicate settlements were discarded.

## `impl InvocationLedger` › `pub fn completed(&self) -> usize {`

How many registrations settled as **completed**.

Kept apart from [`Self::cancelled`] because R3 keeps them apart:
"requested: released on **cancel**" and "granted: released on complete
**or** cancel" are two rows, so a ledger that reported only "settled"
could not tell a process that ran from one that never started, and a
caller that completed a refused spawn would balance and be wrong.

## `impl InvocationLedger` › `pub fn cancelled(&self) -> usize {`

How many registrations settled as **cancelled**.

## `impl InvocationLedger` › `pub fn registered(&self) -> usize {`

How many registrations were ever made, settled or not.

## `impl InvocationLedger` › `pub fn balances(&self) -> bool {`

Whether every registration was settled and the slot table balances — the
process-end condition for R3 and R4 together.

## `impl InvocationLedger` › `pub fn running(&self) -> Vec<&str> {`

The identities running.

## `impl InvocationLedger` › `pub fn pending(&self) -> Vec<&str> {`

The identities waiting for their pair, in identity order (the queue's order
is [`SlotTable::pending`]).

## `mod tests` › `fn every_invocation_of_an_attempt_is_distinct_and_a_retry_reuses_none_of_them() {`

--- assignment --------------------------------------------------------

## `mod tests` › `fn every_invocation_of_an_attempt_is_distinct_and_a_retry_reuses_none_of_them() {`

Every identity of one attempt is distinct, and distinct from every
identity of the next attempt.

ST-04 is "no two … invocations share an InvocationId", and INV-20 adds
"changes with every attempt". Both are asserted over the whole set
rather than pairwise on a sample, because the failure this guards is a
role whose ordinal was forgotten and which therefore collides with its
own neighbour.

## `fn every_invocation_of_an_attempt_is_distinct_and_a_retry_reuses_none_of_them() {` › `assert_ne!(first.gate(0, 0), first.gate(0, 1));`

The ordinal is load-bearing: a gate re-dispatched inside one attempt
is a new identity, so a completion from the first run cannot apply
to the second.

## `fn every_invocation_of_an_attempt_is_distinct_and_a_retry_reuses_none_of_them() {` › `assert_ne!(first.gate(0, 1), first.gate(1, 0));`

And the gate number is load-bearing separately from the ordinal.

## `mod tests` › `fn an_identity_is_a_pure_function_of_its_tuple() {`

The same tuple renders the same identity in any process.

"deterministic in the sequential substrate" is what lets a container
name be predicted, and what makes an intent path stable across the
incarnation that wrote it and the one that reclaims it.

## `mod tests` › `fn a_sequence_has_no_worker_and_shares_no_identity_with_an_attempt() {`

A sequence has gates and reviews and no worker, and its identities do
not collide with an attempt's.

## `mod tests` › `fn a_probe_identity_carries_no_epoch_and_therefore_repeats_across_incarnations() {`

Probe identities repeat across incarnations, deliberately.

This is not a defect to fix here: it is why a container name carries the
coordinator incarnation id. Asserting it keeps the reason visible — a
later change that made probe identities unique per incarnation would
make the incarnation component of a container name dead weight, and this
test is where that shows up.

## `mod tests` › `fn an_agent_probe_refuses_a_name_that_is_not_a_safe_component() {`

An agent name an identity cannot carry is refused, because that identity
becomes a path component and a container-name component.

## `mod tests` › `fn the_synchronous_substrate_refuses_a_pair_it_cannot_grant_at_once() {`

--- slot pairs --------------------------------------------------------

## `mod tests` › `fn the_synchronous_substrate_refuses_a_pair_it_cannot_grant_at_once() {`

The substrate's registration refuses a pair a leaked hold keeps, with the
packet's sentence, and leaves nothing waiting.

This replaces `a_second_slot_pair_is_refused_rather_than_queued`, which pinned
"rather than queueing" — a sentence about an implementation. The pin is now
INV-18's own words, so a later change that makes the substrate wait fails
here for the reason the packet gives.

## `mod tests` › `fn a_gate_and_the_shell_probe_are_refused_a_slot_pair() {`

A gate and the shell probe are refused a slot pair, and every slotted role
is refused a registration without one.

`permits.agent_pool_slots` excludes both by name. Asserted over every
shape of identity rather than one, because the rule is three separate
exclusions — `AttemptRole::Gate`, `SequenceRole::Gate`,
`ProbeTarget::Shell` — and a check that knew only the first would pass a
suite testing only attempts.

## `fn a_gate_and_the_shell_probe_are_refused_a_slot_pair()` › `for (label, id) in [`

And the four slotted shapes cannot register without their pair, so the
refusal is a rule in both directions rather than a blanket.

## `mod tests` › `fn waiting_pairs_are_granted_in_arrival_order_as_their_slots_are_released() {`

FIFO: a release grants the head, not a later arrival.

## `mod tests` › `fn a_waiting_head_reserves_every_slot_it_needs_so_no_later_request_starves_it() {`

The head reservation, on the case it exists for: the head waits for two
slots that free at different times, and a later request that fits the first
one to free is kept off it until the head can take both.

## `mod tests` › `fn a_later_request_runs_past_a_waiting_head_only_when_it_takes_nothing_the_head_needs() {`

The backfill half of the rule, and its edge: the head's pool slot and agent
slot are refused to later requests, a disjoint pair and an agent without a
pool run past it, and a release grants the head before the next arrival.

## `mod tests` › `fn a_cancelled_head_recomputes_the_reservation_and_a_duplicate_cancel_is_counted() {`

G6's "cancelled head request recomputes head reservation; duplicate cancel
ignored and counted", at the ledger.

## `mod tests` › `fn a_pending_invocation_cannot_complete_and_cancelling_it_withdraws_it() {`

A request that never ran cannot complete; cancelling it removes it from the
queue.

## `mod tests` › `fn a_duplicate_settlement_releases_no_pair_twice_and_no_count_goes_negative() {`

ST-05 at the ledger: duplicates after a release move nothing, and the second
holder's slot survives them.

## `mod tests` › `fn slot_limits_of_zero_are_refused_and_the_default_follows_max_parallel() {`

The limits' floor, and the packet's default.

## `mod tests` › `fn a_reservation_is_asserted_singly_and_settles_exactly_once() {`

--- provisional reservations ------------------------------------------

## `mod tests` › `fn a_reservation_is_asserted_singly_and_settles_exactly_once() {`

The sequential form takes one at a time; a reservation settles once, and a
later conversion or cancellation of it is counted, not applied.

## `mod tests` › `fn a_reservation_settled_under_the_wrong_name_is_refused() {`

A settlement naming another task or another kind is refused.

This is the shape that would count an entitlement against the wrong
generation, which is the accounting INV-22 asks to balance. A refusal is not
a duplicate and is not counted as one.

## `mod tests` › `fn cancel_any_releases_an_unnamed_reservation_and_reports_whether_there_was_one() {`

The append-error protocol cancels what it holds without naming it.

## `mod tests` › `fn the_invocation_ledger_refuses_aliasing_and_counts_duplicate_settlements() {`

--- the invocation ledger ---------------------------------------------

## `mod tests` › `fn the_invocation_ledger_refuses_aliasing_and_counts_duplicate_settlements() {`

Registered once, settled once; a duplicate settlement is counted, not
refused.

## `fn the_invocation_ledger_refuses_aliasing_and_counts_duplicate_settlements() {` › `ledger`

"duplicate complete/cancel ignored and counted" — INV-20 asks for a
discard with a warning, not a refusal.

## `mod tests` › `fn settling_an_unregistered_invocation_is_refused() {`

Settling something never registered is refused, and is not a duplicate.

## `mod tests` › `fn cancel_all_running_settles_every_in_flight_invocation() {`

The append-error protocol's half: every running registration is cancelled,
the waiting one is withdrawn without being granted, and the ledger then
balances.
