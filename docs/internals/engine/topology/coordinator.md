# `src/engine/topology/coordinator.rs`

Extended notes for [`src/engine/topology/coordinator.rs`](../../../../src/engine/topology/coordinator.rs).

The code is the authority for what it does; this file is the whole of its prose but for the
concurrency protocol standards §10 places at its site, which the source keeps in four comments:
above `PipelineSeams` (what the shared handles are shared for), `SnapshotGate` (the snapshot gate),
`Coordinator` (the coordinator's protocol) and `Client` (a pipeline's side of it). Each section
here is headed by the line of code it describes, spelled as it is in the source, so the heading is
the grep string that finds the code.

## Module

PR11 phase 3: the Tokio coordinator. [`TopologyRun::run_concurrently`] drives a schema-4 run at
width > 1 through the same transition functions [`TopologyRun::step`] drives at width 1 (`run.rs`),
with an attempt's body and a verification's body running as *pipelines* on Tokio's blocking pool.
It is additive: `step`, `resumed` and `RunSeams` keep their signatures, and nothing in production
calls this entry yet — `upstroke run` still writes schema 3, and a schema-4 run is reachable only
from tests until PR12.

The readings this module implements are the working record's R-R to R-AD
(`reviews/2026-09-30-pr11-record.md`, "Phase 3's readings"); the sections below name them where
they bind.

### Threads (R-R)

The coordinator is the caller's thread. It never enters the runtime: its only waits are
`blocking_recv` on the channel its pipelines send to (and, when a test's observer is present, a
`try_recv` of the injector's), and the `block_on` that joins every pipeline's handle at the end.
Pipelines run on `spawn_blocking` and drive the Runner through `run_blocking_with`, which polls the
Runner's future to completion on the pipeline's own thread — the same substrate `step` uses. The
one thing a pipeline's thread lacks is the run's cleanup scope, which is thread-local; the pipeline
carries the scope's lease paths into every call instead (R-Z: `Carried`,
`RunnerCall::holding_cleanup_leases`).

Tokio is `=1.53.1` with `rt`, `rt-multi-thread` and `sync` only. The runtime has one worker thread,
on which nothing runs, and at most `max_parallel` blocking threads — the fold's pipeline
entitlements bound the live pipelines at that, a verification included. Every thread is named
after the caller's thread, because the test observation export attributes a record to the current
thread's name: a pipeline's records are the test's that started it.

### The cut (R-S)

Selection, admission, dispatch, `attempt_started`, every append, every settlement and every
integration run on the coordinator's thread. The attempt's body (the worker, the capture, the
assessment, the gates and reviews — [`attempt_body`]) and the verification's body
([`verification_body`]) run on pipelines, and append nothing. Dispatch and `attempt_started` stay
together on the coordinator so the fold never shows a generation `OpenNoAttempt` while a pipeline
creates its worktree, where `eligible_continuation` would select it a second time.

### Budget overshoot

`select` checks the ceiling against the **settled** spend only, and a pipeline's spend is known
when its completion is settled — a verification's review spend when its completion is accepted,
before the next selection (R-AH). So a run can overrun its ceiling by the unknown spend of at most
`max_parallel` live pipelines, each running one invocation at a time: the breach is seen at the
next selection after a settlement crosses the ceiling, and the pipelines live at that moment are
not stopped — a budget stop drains them. This bound is stated here only: the report carries no
width and admits no field this slice would add, and it is never a durable field
("durable_events: none new").

### The run's end (phase 4)

The closure under concurrency is the working record's R-AF..R-AK. A halt cancels every live
pipeline at once, waits for each to end, and hands the closure the identities it cancelled
(`closure::Cancelled`); the closure settles exactly those interrupted and completes promotions and
authorized publications. A budget stop admits nothing and drains: live pipelines settle as they
would have, and the closure runs once none is live; a halting settlement drained while others are
live makes the next admission pass cancel them, so the closure runs as Halted. A cancelled
pipeline whose Runner could not establish that its process ended keeps the closure out: the
command ends resumably and names it. A shutdown cancels and drains the same way and appends
nothing. An append error runs the protocol in `emit.rs` and ends the command with its report.

## `pub type HooksFactory = Arc<dyn Fn() -> Box<dyn TopologyHooks + Send> + Send + Sync>;`

Each pipeline's own hooks, made for it at spawn: a pipeline's effects go through hooks it owns,
since the coordinator's hooks are the coordinator's thread's.

## `pub type Reply = oneshot::Sender<Result<(), UpstrokeError>>;`

The coordinator's answer to a pipeline's request — a grant or a refusal — on the oneshot the
request carried.

## `pub struct PipelineSeams {`

What a spawned pipeline takes to its thread: owned seams, where `RunSeams` borrows. The source
keeps why each `Arc` is shared beside the struct (standards §6, §10). `slots` are the command's
`[engine]` slot limits (R-K), installed in the broker before anything is admitted.

## `pub struct PipelineId(pub u64);`

A pipeline's number within this call, from 1 in spawn order. The deterministic intake orders
buffered messages by it.

## `pub enum Identity {`

What a pipeline was spawned for: an attempt (its key, generation and attempt number), or a
verification (its sequence and candidate). Every message a pipeline sends names its pipeline, and a
completion names the identity too, so the coordinator can check the two agree (R-AA).

## `impl Identity` › `fn owns(&self, invocation: &InvocationId) -> bool {`

Whether `invocation` belongs to this identity: an attempt's invocations share its key, generation
and attempt; a verification's share its sequence. An invocation offered or ended by a pipeline that
does not own it is refused and counted.

## `impl Identity` › `fn open_in(&self, run: &TopologyRun) -> bool {`

Whether the fold still holds this identity open: an attempt's worker standing is a pipeline
entitlement's, a verification's gate standing holds the pipeline and merge entitlements and the
open transaction is its candidate's. A completion for an identity the fold does not hold open is
stale, and is discarded.

## `pub enum ToCoordinator {`

The protocol's messages. `Admit` and `SnapshotBegin` are requests, answered on their `reply`;
`Ended` and `SnapshotEnd` are notifications; `Judged` and `Verified` are a pipeline's completion,
its last message; `Shutdown` is the command's (a test's, until PR12 wires signals). A `Judged`
outcome is boxed because a judgement is large and the channel's other messages are small.

## `pub trait Quiescence {`

A test's hook into the deterministic intake (R-AD), called at two points. `granted` is handed every
invocation the coordinator grants, the moment the grant is sent and before the coordinator does
anything else, and returns once the invocation is inside the Runner: the coordinator cannot see into
the Runner, and the test's runner is the one party that can, so the observer is where a grant is
settled. `quiescent` is called when every live pipeline is waiting on the coordinator or on an
invocation inside the Runner and nothing is buffered, and chooses what happens next.

Without the first, a pipeline between its grant and its Runner call was running on its own while
the coordinator applied the next message, appended, cancelled or asked the observer — phase 3's CI
failure at `aea75a79` (the working record's §13, round C1): a shutdown injected at the first
quiescent point cancelled workers that had not yet reached the Runner, and a seed did not reproduce
a run.

## `pub enum Release {`

The observer's choice: one held invocation to let finish (the observer delivers its result before
it returns), a message it injected, or nothing — which ends the command as stuck.

## `pub struct Quiescent<'a> {`

What the observer sees: the invocations the live pipelines are running, the live pipelines and
their identities, the run, and the injector.

## `pub struct Injector(mpsc::UnboundedSender<ToCoordinator>);`

How a test forges a message — a stale, duplicate or mismatched completion — to prove the
coordinator discards it. An injected message is never trusted as a pipeline's: it is checked, and
anything it would settle or release is refused.

## `impl TopologyRun` › `pub fn run_concurrently(`

Run the schema-4 loop at the run's width until it finishes, or until the command ends.

It enters the run's cleanup scope on the caller's thread, reads the scope's lease paths once (Unix;
elsewhere there are none) for every pipeline to carry, installs the command's slot limits, builds
the runtime and drives. It joins every pipeline's handle before it returns, whatever the outcome.

### Errors

A refusal before anything is spawned: slot limits that cannot replace the broker's (something is
outstanding in it), or a runtime that could not be built. After that, whatever ends the command: a
pipeline's error or panic, a coordinator-side error (a settlement's Git failure, a refused
dispatch, a poisoned fold), an append error (its protocol's report), a shutdown, a halt whose
cancelled process the Runner could not establish as ended, or a closure step's own error. Every one
of them ends the command resumably: live pipelines are cancelled and waited for, what they return is
discarded, and no `attempt_interrupted` is appended by an error or a shutdown — the next resume
settles the open attempts interrupted (R-AB). Only a halt's closure appends it, for the attempts it
cancelled (R-AF).

## `enum Busy {`

What a live pipeline is doing as the coordinator knows it, updated at the receipt of each message:
`Running` on its own, `Awaiting` a reply, `Invoking` a granted invocation, `Done` once its
completion is received. With an observer, the coordinator applies buffered messages only when no
pipeline is `Running`, and a pipeline becomes `Invoking` only once the observer has seen its
invocation inside the Runner, which together make the order it applies them in, and what each
process sees when it starts, independent of thread timing.

## `struct Live {`

A live pipeline's entry: its identity, its cancellation, whether it was cancelled, what it is doing,
the invocation it is running (to withdraw from the broker if it ends holding it), and, for an
attempt, the job the coordinator settles against.

## `enum Interrupt {`

Why the command is ending: a halting settlement (`Halt`), a shutdown, or an error. The first one
recorded is the one the command ends with; a later error is a warning.

## `enum Origin {`

Whether a message came from a pipeline or through the injector. An injected request, end or
completion never moves a pipeline's `Busy` state and never settles or releases anything; an
injected `Shutdown` is the command's own.

## `enum GateMode {`

The snapshot gate's three modes (R-W): `Open` grants at once; `Closed` queues; `Verifying` grants
until the verification's completion has arrived and queues after it.

## `struct SnapshotGate {`

The source keeps the gate's protocol beside the struct (§10). Why it exists: the frozen
`integrate()` reclaims **every** snapshot intent of the execution root on a stale terminal
(`integrate.md`, `reclaim_snapshots`), a rationale that rests on one judgement at a time. The gate
is isolated in this one type so that, should the owner later take option A (a three-line edit of
the frozen file; R-W), it can be deleted without touching anything else.

## `impl SnapshotGate` › `fn take_granted(&mut self) -> Vec<(PipelineId, Reply)> {`

Grant every waiting pipeline when the gate grants, in the order they asked.

## `struct Coordinator<'s> {`

The source keeps the coordinator's protocol beside the struct (§10): the owner of every shared
state, the linearization point, a pipeline's transitions, which completion wins, the cleanup after
an interrupt, and why the unbounded channels are bounded.

## `impl Coordinator<'_>` › `fn drive(&mut self) -> Result<Progress, UpstrokeError> {`

Admit everything selection admits; end the command on an interrupt; when nothing is live, take the
idle-only arms; otherwise apply the next message. An error anywhere is recorded as the interrupt
and the loop ends through [`Self::finish`].

## `impl Coordinator<'_>` › `fn admit(&mut self) -> Result<(), UpstrokeError> {`

Select and start until selection has nothing to start now. Each pass reads the fold afresh
(INV-21): a poisoned fold refuses; an ending run stops admission — a halt with pipelines live
cancels them, a budget stop lets them drain (R-AC); a draining gate waits; an answer ingested
restarts the pass. Then `admitted()` — the one selection `step` makes too — and its arm: a budget
breach appended, an integration run, a retry or a dispatch started and spawned. Backoff, hard block
and closure are taken only when no pipeline is live (R-AB), so with pipelines live they end the
pass; the cost is latency, and a `defer_wait_elapsed` is never placed among in-flight settlements.

## `impl Coordinator<'_>` › `fn integrate(&mut self, candidate: CandidateRef) -> Result<bool, UpstrokeError> {`

An integration, on the coordinator's thread, through the frozen `integrate()` over
[`DrivenJournal`]. Stale is predicted before anything is taken — the candidate's recorded base is
not the log's authorized head, which is `decide`'s own test once the ref is not foreign (a foreign
head refuses before any staging or reclaim) — and while an attempt snapshot is live a stale
integration takes no reservation: the gate closes, admission stops, messages are applied, and when
no attempt snapshot is live selection runs afresh (an answer ingested meanwhile can put another
candidate at the head of the queue). A fast integration never reclaims and does not wait. `false`
ends the admission pass: nothing was started, or the command is already ending — an error the
interrupt already accounts for is not a second error.

## `impl Coordinator<'_>` › `fn idle(&mut self) -> Result<Progress, UpstrokeError> {`

The idle-only arms, by the functions `step` runs: the backoff, the hard block, the closure. Any
other arm here is a refusal, because the admission pass takes it first.

## `impl Coordinator<'_>` › `fn spawn_attempt(&mut self, job: AttemptJob) {`

An attempt's body on a pipeline: its standing read from the fold once, after `attempt_started`; a
[`Carried`] with its own cancellation and the run's lease paths; a gated `Client`; its own hooks; a
copy of the job (the coordinator keeps the original to settle against). A panic inside the body is
caught and becomes the pipeline's completion (`panicked`), so a pipeline always reports.

## `impl Coordinator<'_>` › `fn spawn_verification(&mut self, job: VerificationJob) -> PipelineId {`

A verification's body on a pipeline, with an ungated `Client` — the verification's snapshots are
the integration's own, the ones the reclaim is for — and a [`SpendAccount`] with no `Spend`: its
review passes come back with its completion and are charged by the coordinator.

## `impl Coordinator<'_>` › `fn verify_concurrently(`

The coordinator's `verify`, re-entrant (R-V): the frozen `integrate()` calls
[`Verification::verify`] on the [`DrivenJournal`], which lands here. It spawns the verification
and keeps running the loop — admission, settlements, dispatch, promotion — until the
verification's completion arrives and no attempt snapshot is live, then charges the passes and maps
the outcome through [`verified`], the width-1 mapping. The gate grants during the verification and
stops granting once its completion has arrived, so every snapshot the reclaim that follows removes
is the integration's own.

### Errors

A refusal when asked inside another verification (one transaction is open at a time); the
verification job's own refusals; and, when an interrupt ends the command first, a refusal naming
it — the pipeline was cancelled, and `integrate()` appends nothing further for it. The sequence is
recorded as cancelled, whether or not its completion had already arrived, so a halt's closure
settles the transaction with `merge_verification_interrupted` (R-AF); an arrived completion whose
Runner left its process unresolved is recorded as such, and keeps the closure out.

## `impl Coordinator<'_>` › `fn next_message(&mut self) -> Result<(Origin, ToCoordinator), UpstrokeError> {`

Without an observer — the production shape — the next message in arrival order. With one, the
deterministic intake (R-AD): an injected message first; otherwise buffer everything that has
arrived, wait while any pipeline is `Running`, then apply the buffered message that sorts first by
pipeline and arrival; when nothing is buffered, every live pipeline is waiting — for a reply, or
inside the Runner, since [`Self::started`] settled each grant with the observer — and the observer
chooses what happens next.

## `impl Coordinator<'_>` › `fn observe(&mut self) -> Result<(), UpstrokeError> {`

Ask the observer. A released invocation's pipeline is `Running` again; an injection must have put
something in the injector; nothing released is `stuck`, which ends the command resumably.

## `impl Coordinator<'_>` › `fn admit_invocation(`

A pipeline's `Admit`: refused and counted when injected, from a pipeline that is not live, or for
an invocation its identity does not own; refused as cancelled when its pipeline was cancelled or
the command is ending; otherwise registered with the broker against the invocation's standing —
granted at once (the reply is sent), or pending until a slot pair frees (the reply waits in
`replies`). A grant whose pipeline stopped waiting is withdrawn, and whatever that frees is granted.

## `impl Coordinator<'_>` › `fn started(&mut self, origin: Origin, pipeline: PipelineId, invocation: InvocationId) {`

A grant sent: the invocation is the pipeline's running one (withdrawn from the broker if the
pipeline ends holding it), and the pipeline is `Invoking`. With an observer, the grant is first
handed to `Quiescence::granted`, and nothing else happens until it returns with the invocation
inside the Runner; every grant passes through here, those answered at once and those a later
release frees alike.

## `impl Coordinator<'_>` › `fn end_invocation(`

A pipeline's `Ended`: completed or cancelled in the broker, and whatever that frees granted. An end
from a pipeline that is not live, for an invocation its identity does not own, or for an invocation
it is not running and the ledger never settled, is discarded and counted, and releases nothing.

## `impl Coordinator<'_>` › `fn begin_snapshot(`

A pipeline's `SnapshotBegin`: refused when injected, when the pipeline is not live or is being
cancelled, or when the command is ending; granted at once when the gate grants; queued otherwise.

## `impl Coordinator<'_>` › `fn check(&mut self, origin: Origin, pipeline: PipelineId, identity: &Identity) -> Option<Live> {`

The identity check every completion passes before anything is settled (R-AA), in order: a poisoned
fold discards it silently (the command is already ending); a pipeline that is not live makes it
stale or a duplicate; an identity that is not the pipeline's is a mismatch; a cancelled pipeline's
completion is the expected end of its cancellation and is discarded silently; an identity the fold
no longer holds open is stale; and an injected completion for a pipeline still running is
discarded. Only then is the pipeline retired and its entry returned. Every discard is counted
(`TopologyRun::discarded`), and all but the silent ones are warned about.

## `impl Coordinator<'_>` › `fn retire(&mut self, pipeline: PipelineId) -> Option<Live> {`

Remove a pipeline: its snapshot grants and waits released, and an invocation it still holds
withdrawn from the broker, whatever that frees granted.

## `impl Coordinator<'_>` › `fn judged(`

An attempt's completion, checked and then settled by `settle_judged`, the function `step` settles
through. A pipeline's error ends the command (R-AB).

## `impl Coordinator<'_>` › `fn verified_arrived(`

A verification's completion, held for `verify_concurrently` once checked, its review records charged
to the run's spend at once, so the next selection's ceiling check counts them (R-AH; the early
review's `R1-CONC-4`). One that no open verification awaits is a cancelled pipeline's end (silent)
or stale (warned).

## `impl Coordinator<'_>` › `fn note_cancelled_end(`

Read a cancelled pipeline's completion before it is discarded: an error whose Runner fate is
unresolved names a process that may still run, and is recorded (R-AF).

## `impl Coordinator<'_>` › `fn cancel_all(&mut self) {`

Cancel every live pipeline's token, withdraw every pending registration, and refuse every reply
still owed — so no pipeline waits on a coordinator that is ending, and every one of them reaches its
completion. Registrations already granted are released as their pipelines end them. Each identity
cancelled here is recorded, for a halt's closure to settle.

## `impl Coordinator<'_>` › `fn finish(&mut self) -> Result<Progress, UpstrokeError> {`

End the command: cancel, apply messages until no pipeline is live, then act on the interrupt.
After a halt the closure runs with the identities this coordinator cancelled — unless a cancelled
pipeline ended with its process unresolved, in which case nothing is appended and the command ends
resumably naming it. A shutdown cancels any provisional reservation still held (none is expected)
and ends resumably, appending nothing. An error is returned as it is: after an append error it is
the protocol's report, and no closure, report or cleanup follows.

## `struct Coordinator<'s>` › `cancelled_work: closure::Cancelled,`

The in-flight identities this coordinator cancelled or abandoned, which a halt's closure settles.

## `struct Coordinator<'s>` › `unresolved: Vec<String>,`

The invocations a cancelled pipeline reported with an unresolved process fate.

## `fn unresolved_runner(error: &UpstrokeError) -> Option<String> {`

The invocation an error names when its process fate is unresolved, with `unresolved_judge` for the
verification's error type.

## `impl Drop for Coordinator<'_>` › `fn drop(&mut self) {`

Cancel whatever is still live and drop every reply still owed, so that a coordinator unwinding
leaves no pipeline waiting on it.

## `impl Driver for Coordinator<'_>` › `fn verify(&mut self, request: &VerifyRequest<'_>) -> Result<Verified, UpstrokeError> {`

The frozen `integrate()`'s verification, run concurrently.

## `struct Client {`

The source keeps a pipeline's side of the protocol beside the struct (§10). `gated` is true for an
attempt's pipeline and false for a verification's.

## `impl Client` › `fn ask(&self, message: impl FnOnce(Reply) -> ToCoordinator) -> Result<(), UpstrokeError> {`

Send a request and wait for its reply on the pipeline's thread. A coordinator that has stopped
listening is a refusal, never a hang.

## `impl Registrar for Client {`

A pipeline's registrar (R-U): `admit` asks and waits for the grant, `ended` notifies, and the two
snapshot calls ask and notify when the pipeline is gated.

## `mod tests`

The scheduler first: `Seeded` is a splitmix64 stream; a `Scheduler` is a [`Quiescence`] observer
releasing the first held invocation, a seeded choice, or what a script chooses, and records the
widest set of invocations — and of slotted invocations — it saw granted at once. The tests hold
every invocation (`RecordingRunner::hold`) and answer it with the fixture's responder, so the order
of completions is the scheduler's and never the threads'.

A scheduler also owns the entry of every call into the runner. It puts its runner in late entry
(`RecordingRunner::enter_late`), so no granted call starts until the scheduler admits it, and it
admits each one in `granted` (`RecordingRunner::admit`), so every process starts while the
coordinator waits there and at no other time. At every quiescent
point it first checks that each invocation it is handed is inside the runner
(`RecordingRunner::inside`). A coordinator that acted again before a granted pipeline reached the
Runner therefore fails every scheduler-driven test at its first quiescent point, on any machine —
not only when a slow one happens to widen the window, as CI's did at `aea75a79`.

## `mod tests` › `fn canonical(events: &[TopologyEvent], run_id: &str) -> Vec<serde_json::Value> {`

The packet's `canonical_trace_projection`, for schema-4 events: drop the run's identity and
environment and the scheduling configuration, keep everything else, and label every commit SHA by
first appearance (tree OIDs literal).

## `mod tests` › `fn two_independent_tasks_overlap_merge_through_one_queue_and_the_dependent_starts_on_both() {`

Acceptance item 2, with `disjoint_hints_dispatch_together_overlapping_and_absent_hints_serialize`.

## `mod tests` › `fn the_coordinator_settles_promotes_and_dispatches_while_a_verification_is_open() {`

R-E and R-V: the coordinator inside `verify`, with `halt_interrupts_verification`.

## `mod tests` › `fn halt_cancels_in_flight_attempt_at_width_three() {`

T-ATTEMPT at width three: a decline halts the run while alpha is at its gate and beta at its worker;
both are terminated, each released once after its termination, and the closure appends one
`attempt_interrupted` per attempt, then reclaims its snapshot and worktree, before `run_finished`.
`Watching` records the execution root's intents after every append, which is how the order of
terminal and reclaim is read. With `halt_interrupts_verification` (T-VERIFY: a halt inside
`verify`, settled by `merge_verification_interrupted`, the pin deleted and this sequence's staging
and snapshots reclaimed before the end), the phase-3 test that held the closure's refusal is gone.

## `mod tests` › `fn a_shutdown_with_pipelines_in_flight_is_settled_by_the_next_resume_whose_ledgers_start_empty()`

G6's shutdown row, resumed through the frozen recovery order itself (`Wide::durable`,
`Wide::resume`, `scaffold.md`).

## `mod tests` › `fn append_error_under_concurrency_cancels_pipelines_and_folds_nothing_from_memory() {`

The slice contract's named test, at each T-APPEND error-return shape — a partial write, a flush
error after the full line, a sync error — injected at the settlement of one pipeline while two
others are in flight, and the resume following the surviving prefix (R-AI).

## `mod tests` › `fn kill_inside_closure_recovers_at_width_three() {`

T-FINISH at width three, with
`append_error_inside_closure_ends_command_and_resume_completes_closure_at_width_three`: `HaltArming`
arms the fault at the first append after the halt is folded, which is the closure's first terminal;
the child (`closure_kill_child_at_width_three`) dies inside it, torn or complete, and the next
process repeats the closure from the surviving prefix.

## `mod tests` › `fn prepared_publication_completed_at_run_end() {`

T-PREPARED's closure half, on a state built by making the fast integration's CAS fail once (R-AG).

## `mod tests` › `fn over_budget_prefix_without_budget_exceeded_is_not_ending_at_width_three() {`

With `a_budget_stop_drains_live_pipelines_to_their_settlements_and_ends_budget_exceeded`, G6's budget
row: `budget_exceeded` before any budget-driven end, the drain, the end. `Recording` keeps the fold
after every append, and `ending_as` asks it what the fold would say to a `run_finished`.

## `mod tests` › `fn run_finished_budget_exceeded_refused_after_halting_drain_settlement_at_width_three() {`

In this build a halting settlement is a declined answer, which the fold refuses while a budget stop
is current, so a halting drain settlement is planted on a width-three prefix (R-AH).

## `mod tests` › `fn a_halt_whose_cancelled_process_is_unresolved_ends_the_command_without_closing() {`

R-AF's guard, the phase-4 side of the early review's `R1-CONC-1`: the double reports a cancelled
worker's process unresolved (`RecordingRunner::unresolved_when_cancelled`), and the closure is not
entered.

## `mod tests` › `fn a_closure_never_settles_in_flight_work_its_coordinator_did_not_cancel() {`

R-AF's vouching, from the other side: a second coordinator in the same process, which cancelled
nothing, meets a halt with two attempts still in flight from the first; its closure refuses them,
appending nothing, and the next process's recovery settles them.

## `mod tests` › `fn a_verifications_review_spend_is_charged_before_the_next_selection() {`

R-AH's current spend, the phase-4 side of the early review's `R1-CONC-4`: a verification's
completion arrives while an attempt snapshot keeps `verify` waiting, a third task settles, and the
selection it opens meets a ceiling only the verification's review spend has crossed.

## `mod tests` › `fn stale_duplicate_and_mismatched_completions_are_discarded_with_a_warning_and_counted() {`

ST-01, ST-02 and ST-06, through the injector; `a_shutdown_cancels_every_live_pipeline_and_ends_the_command_resumably`
follows them.

## `mod tests` › `fn a_shutdown_cancels_every_live_pipeline_and_ends_the_command_resumably() {`

R-AB's shutdown, injected at the first quiescent point with three workers granted — the
interleaving phase 3's CI failed on (the working record's §13, round C1). It states the packet's
`cancellation` sentence — a shutdown "cancels/releases granted and non-slotted invocations after
termination" — on both sides: in the runner every granted worker's process had started and was
terminated as a cancellation; in the ledger each was released exactly once, as a cancellation, by
its pipeline's own report of the end. A release before the termination would make that report a
counted duplicate.

## `mod tests` › `fn a_shutdown_releases_each_invocation_once_whether_pending_unstarted_running_or_finished() {`

Every place a cancellation can find an invocation, at one shutdown: four workers at width four with
three agent slots, so one request is pending; one granted worker is barred at the runner's door
(`RecordingRunner::bar`: granted, its process not started — the interleaving the deterministic
intake no longer produces by itself, and production can); one is running; and one has just finished,
its result delivered and its end not yet reported. Each is released exactly once: the pending one
withdrawn without reaching the runner, the unstarted one cancelled before it started, the running
one terminated, the finished one completed. The ledger balances with no duplicate (R3, R4; R-AE).

## `mod tests` › `fn one_seed_reproduces_one_run_down_to_what_each_process_saw_when_it_started() {`

R-AD's claim, pinned: three runs of one seed release the same invocations in the same order, start
the same processes in the same order on the same workspaces with the same log durable at each
start, and write the same canonical log.

## `mod tests` › `fn out_of_order_completions_bind_to_their_own_identities_under_seeded_permutations() {`

ST-03 and ST-08 under seeded permutations, with
`independent_tasks_dispatch_together_and_keep_per_key_projections_under_every_seed` and
`a_chain_plan_projects_identically_at_widths_three_and_one`.

## `mod tests` › `fn concurrent_attempts_at_one_generation_and_attempt_use_distinct_snapshot_slots() {`

ST-04 — at most one provisional reservation, read through `Reservations::peak` — with distinct
snapshot names (R-Y); `live_state_equals_replay_after_every_append_at_width_three` is ST-10.

## `mod tests` › `fn a_retained_retry_is_admitted_beside_other_pipelines_and_regates_on_fresh_snapshots() {`

ST-15.

## `mod tests` › `fn adversarial_orders_with_one_slot_per_agent_and_pool_always_reach_run_finished() {`

Deadlock freedom: one slot per agent and per pool, seeded orders, every run reaches
`run_finished`.

## `mod tests` › `fn a_pipeline_error_cancels_the_others_and_ends_the_command_resumably() {`

A pipeline that fails, with `a_panicking_pipeline_ends_the_command_with_a_defined_error`.
