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

The readings this module implements are the working record's R-R to R-AL
(`reviews/2026-09-30-pr11-record.md`, "Phase 3's readings" and "Phase 4's readings"), as the
early review's repairs (§13, round R1) and review round 2's (§13, round R2) corrected them; the
sections below name them where they bind.

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

### What an end establishes, and what closes an identity (round R1)

The early review of phases 0–3 (the working record's §13, round R1) found the coordinator acting on
less than it had been told. As repaired:

- **An invocation is released only when its end established that its process is gone.** `Ended`
  carries the Runner's process fate (`InvocationEnd`). An invocation whose Runner could not
  establish that its process ended (`ProcessFate::Unresolved`), or whose pipeline never reported
  its end (a panic unwound through the Runner call), keeps its registration and its slot pair for
  the rest of this process, so nothing is granted on them — R3 and R4 release a granted invocation
  after its termination, and for such a process that is this process's death — and it interrupts
  the command at once, which cancels every live pipeline and withdraws every request that could
  otherwise wait for that pair for ever (`R1-CONC-1`).
- **A fatal completion interrupts as it is received**, inside `verify` as outside. A verification's
  completion is mapped at receipt by the width-1 mapping (`verified`); an error ends the command
  there and then, and only an accepted result waits for the snapshot drain its terminal needs
  (`R1-CONC-2`).
- **Every admission pass first stops each pipeline whose identity the fold has closed** — a decline
  that does not halt fails its lineage, and so does a lineage member's failed settlement — and
  `verify` unwinds when its transaction disappears, abandoning the integration without ending the
  command (`R1-CONC-3`).
- **A verification's review spend is charged when its completion is accepted**, before the next
  selection (phase 4; `R1-CONC-4`).

### What a halt interrupts, where admission stops, and whom a grant reaches (round R2)

Review round 2 (the working record's §13, round R2) found three places where the coordinator read its
own table where the fold is the authority. As repaired:

- **A halt interrupts every in-flight identity the fold shows**, not only the live pipelines. A
  verification whose result has arrived — its pipeline retired, its judgement held for `verify` — is
  still in flight until `integrate()` appends its terminal, so a halt recorded then ends `verify` as
  interrupted, and the closure settles the transaction with `merge_verification_interrupted`, its pin
  deleted expected-old and its sequence's staging and snapshots reclaimed; it is never prepared or
  published (`R2-CONC4-1`). A budget stop, which drains, still lets it reach its natural terminal.
- **While `verify`'s own transaction is gone, an admission pass selects nothing** — no integration,
  retry, dispatch, budget stop or answer — until `verify` has seen its pipeline end and unwound; the
  pass after it goes on outside `verify`, and a candidate already queued is integrated next
  (`R2-DECLINE-QUEUED`). Round R1's repair held only for a candidate that became eligible after
  `verify` had unwound.
- **A grant reaches only a pipeline that receives it**: live, not cancelled, its identity open in
  the fold, and no interrupt recorded (`receives`). A pair freed for any other — by the stop of
  another closed pipeline, by an end, by a withdrawn grant — is withdrawn undelivered and its request
  refused, so no process starts for an identity the fold has closed; a registration or a snapshot such
  a pipeline asks for, or waits for, is refused the same way (`R2-RECONCILE-GRANT`).

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
outcome is boxed because a judgement is large and the channel's other messages are small. `Ended`
carries how the invocation's Runner call ended (`InvocationEnd`): completed, or failed with the
process fate the Runner established and its account of why — which is what tells a process that is
gone from one that may still run (round R1, `R1-CONC-1`).

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
it returns), a message it injected, an event to append, or nothing — which ends the command as
stuck.

`Append` plants a recorded event: the coordinator appends it through its own emitter, every fold
check applying, as a transition it made, and carries on. The live engine of this build does not
reach every state the fold admits — it spawns a repair only from a rejection and never runs two
members of one lineage at once — and the coordinator must still handle what the fold admits, so the
tests that need such a state (a sibling repair's embedded question, round R1's `R1-CONC-3`) plant it
at a quiescent point. Production passes no observer.

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
(INV-21): a poisoned fold refuses; every pipeline whose identity the fold no longer holds open is
stopped (`reconcile`), before anything else is decided; an ending run stops admission — a halt with
anything in flight (`any_in_flight`) interrupts it, a budget stop lets it drain (R-AC, R-AH); while
`verify`'s transaction is gone the pass ends there, so `verify` unwinds before anything else is
selected (`abandoning`, round R2); a draining gate waits; an answer ingested restarts the pass. Then `admitted()` — the one selection `step` makes too — and its arm: a budget
breach appended, an integration run, a retry or a dispatch started and spawned. Backoff, hard block
and closure are taken only when no pipeline is live (R-AB), so with pipelines live they end the
pass; the cost is latency, and a `defer_wait_elapsed` is never placed among in-flight settlements.

## `impl Coordinator<'_>` › `fn any_in_flight(&self) -> bool {`

Whether a halt has anything to interrupt: a live pipeline, or in-flight work the fold shows
(`closure::in_flight`). With no pipeline live, that is the verification `verify` still holds after its
result arrived. The phase-4 guard read the live table alone, so a halt ingested in the pass after the
verification's result was accepted — its pipeline already retired — was not acted on: `verify`
returned the result and `integrate()` prepared and published it after the halt (round R2,
`R2-CONC4-1`). In-flight work the fold shows that no pipeline of this coordinator served is not
settled by the halt either: the closure refuses what it is not vouched for.

## `impl Coordinator<'_>` › `fn abandoning(&self) -> bool {`

Whether `verify` is on the stack and the fold no longer holds its transaction open — a decline, or a
lineage member's failed settlement, failed the lineage, and `reconcile` has stopped the verification's
pipeline. Admission ends its pass while it is, so the next thing `verify` does is wait for that
pipeline to end and unwind. Selection would otherwise see the transaction gone and admit other work
under a `verify` that is still returning; a candidate already queued was selected for integration and
refused inside `verify`, which ended the command (round R2, `R2-DECLINE-QUEUED`).

## `impl Coordinator<'_>` › `fn integrate(&mut self, candidate: CandidateRef) -> Result<bool, UpstrokeError> {`

An integration, on the coordinator's thread, through the frozen `integrate()` over
[`DrivenJournal`]. Stale is predicted before anything is taken — the candidate's recorded base is
not the log's authorized head, which is `decide`'s own test once the ref is not foreign (a foreign
head refuses before any staging or reclaim) — and while an attempt snapshot is live a stale
integration takes no reservation: the gate closes, admission stops, messages are applied, and when
no attempt snapshot is live selection runs afresh (an answer ingested meanwhile can put another
candidate at the head of the queue). A fast integration never reclaims and does not wait. `false`
ends the admission pass: nothing was started, or the command is already ending — an error the
interrupt already accounts for is not a second error. A verification abandoned because the fold
cancelled its transaction (`abandoned`) ends the integration without ending the command:
`integrate()` appends nothing after `verify`'s error, and admission goes on. Its staging worktree,
pin and snapshots are left for the terminal finalization or the next resume's reclaim, which remove
any staging and pin no open transaction owns; the snapshots are its own sequence's, and a later
stale integration's reclaim takes them too.

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
verification's result has been accepted and no attempt snapshot is live, then returns it as it was
mapped at receipt ([`verified`], the width-1 mapping; the passes were charged there too). A fatal
completion never waits for the drain: it interrupts as it is received (round R1, `R1-CONC-2`). The
gate grants during the verification and stops granting once its result has arrived, so every
snapshot the reclaim that follows removes is the integration's own. When the fold stops holding
the transaction open — a decline, or a lineage member's failed settlement, failed its lineage — the
admission pass has stopped the verification; `verify` waits for its pipeline to end and returns an
error that abandons the integration (`R1-CONC-3`), so it never waits on an inbox its verification
will not write to.

### Errors

A refusal when asked inside another verification (one transaction is open at a time); the
verification job's own refusals; and, when an interrupt ends the command first, a refusal naming
it — the pipeline was cancelled, and `integrate()` appends nothing further for it. The sequence is
recorded as cancelled, whether or not its result had already arrived, so a halt's closure settles
the transaction with `merge_verification_interrupted` (R-AF) — a halt recorded after the result
arrived and before this returned included (round R2: the admission pass's halt check reads the fold's
in-flight work, not the live table). And, when the fold no longer holds the transaction open, a
refusal naming the abandonment, which `integrate` reads through `abandoned` and does not treat as the
command's end; admission selects nothing until then (`abandoning`).

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
an invocation its identity does not own; refused as cancelled when its pipeline was cancelled, the
command is ending, or the fold no longer holds its identity open (round R2 — the broker checks the
standing of a slotted request, and a gate or the shell probe has none to check); otherwise
registered with the broker against the invocation's standing — granted at once (the reply is sent),
or pending until a slot pair frees (the reply waits in `replies`). A grant whose pipeline stopped
waiting is withdrawn, and whatever that frees is handed on by `reply_granted`.

## `impl Coordinator<'_>` › `fn receives(&self, pipeline: PipelineId) -> bool {`

Whether `pipeline` may be handed a grant — a slot pair or a snapshot: it is live, not cancelled, the
fold holds its identity open, and no interrupt is recorded. Every place a grant is delivered asks it
(round R2, `R2-RECONCILE-GRANT`).

## `impl Coordinator<'_>` › `fn reply_granted(&mut self, granted: Vec<InvocationId>) {`

Hand on what a settlement granted. The broker grants by its queue alone and knows nothing of the
fold, so each waiting pipeline is asked `receives` first: one that does is sent the grant; any other
is refused, its grant withdrawn from the broker undelivered — no process of it started — and whatever
that frees handed on the same way, with a warning. The phase-3 shape sent every grant: stopping one
closed pipeline withdrew its request, which, at the head of the queue, had reserved a pool another
closed pipeline's request waited for, and the pair went to that pipeline before it was stopped
(round R2).

## `impl Coordinator<'_>` › `fn started(&mut self, origin: Origin, pipeline: PipelineId, invocation: InvocationId) {`

A grant sent: the invocation is the pipeline's running one (withdrawn from the broker if the
pipeline ends holding it), and the pipeline is `Invoking`. With an observer, the grant is first
handed to `Quiescence::granted`, and nothing else happens until it returns with the invocation
inside the Runner; every grant passes through here, those answered at once and those a later
release frees alike.

## `impl Coordinator<'_>` › `fn end_invocation(`

A pipeline's `Ended`: settled in the broker by its fate (`PermitBroker::end`) — completed; cancelled
when the Runner established that no process of it runs; kept, registration and pair, when it could
not — and whatever that frees granted. An unresolved end of the invocation the pipeline is running
is then held (`hold_unresolved`). An end from a pipeline that is not live, for an invocation its
identity does not own, or for an invocation it is not running and the ledger never settled, is
discarded and counted, and releases nothing.

## `impl Coordinator<'_>` › `fn hold_unresolved(&mut self, invocation: &InvocationId, cause: String) {`

An invocation whose Runner could not establish that its process ended — its end was unresolved, or
its pipeline ended without reporting it — keeps its registration and its slot pair for the rest of
this process (R3, R4: released after termination, which for it is this process's death), so nothing
is granted on them. It is recorded in `unresolved`, and unless the command is already ending it is
the interrupt: admission stops and every live pipeline is cancelled at once, which also withdraws
every request waiting for a pair, so no request waits for ever on the pair that is never released.
The command ends resumably and the next process's census reclaims what is left (round R1,
`R1-CONC-1`). The phase-3 shape settled such an end as a cancellation, released the pair, and could
grant it to a waiting invocation whose process then started beside one that might still run.

## `impl Coordinator<'_>` › `fn begin_snapshot(`

A pipeline's `SnapshotBegin`: refused when injected, or when the pipeline does not receive grants
(`receives`: not live, cancelled, its identity closed, or the command ending); granted at once when
the gate grants; queued otherwise.

## `impl Coordinator<'_>` › `fn grant_snapshots(&mut self) {`

Grant every snapshot the gate releases to a pipeline that still receives grants, and refuse the others
(round R2).

## `impl Coordinator<'_>` › `fn accepts(&mut self, origin: Origin, pipeline: PipelineId, identity: &Identity) -> bool {`

The identity check every completion passes before anything is settled (R-AA), in order: a poisoned
fold discards it silently (the command is already ending); a pipeline that is not live makes it
stale or a duplicate; an identity that is not the pipeline's is a mismatch; a cancelled pipeline's
completion is the expected end of its cancellation and is discarded silently; an identity the fold
no longer holds open is stale; and an injected completion for a pipeline still running is
discarded. What it discards it retires where the pipeline is ending (a poisoned fold's, a cancelled
pipeline's, a closed identity's); an accepted completion is retired by its caller, after the caller
has classified it (round R1: a fatal completion interrupts before anything is retired or granted).
Every discard is counted (`TopologyRun::discarded`), and all but the silent ones are warned about.

## `impl Coordinator<'_>` › `fn retire(&mut self, pipeline: PipelineId) -> Option<Live> {`

Remove a pipeline: its snapshot grants and waits released. An invocation it still holds is one it
never reported ended — a panic unwound through the Runner call — so nothing established that its
process ended: it keeps its registration and pair (`hold_unresolved`), where the phase-3 shape
withdrew it and could grant its pair on (round R1, `R1-CONC-1`).

## `impl Coordinator<'_>` › `fn reconcile(&mut self) {`

After an append that can close an identity a live pipeline serves — an ingested decline fails its
lineage (`design/26_design_merge_queue_protocol.md`: "A decline fails every unmerged lineage member
… A matching `VerificationStarted` transaction is cancelled"), and so does a lineage member's
failed settlement — each live, uncancelled pipeline whose identity the fold no longer holds open is
stopped. The design's words for the concurrent driver: it "must stop the affected work and discard
late results before appending their completion". Run first in every admission pass, which follows
every message applied and every answer ingested, inside `verify` as outside (round R1,
`R1-CONC-3`). In this build's live engine no such pipeline exists — at most one member of a lineage
is ever live, and a lineage question blocks the others' dispatch, start and integration — but the
fold admits the state, and the design requires it handled. When several close at once, stopping one
can free a pair another's waiting request is granted; `reply_granted` withdraws that grant undelivered,
so none of them starts a process (round R2, `R2-RECONCILE-GRANT`).

## `impl Coordinator<'_>` › `fn stop(&mut self, pipeline: PipelineId) {`

One pipeline's cancellation, outside an interrupt: its token cancelled (the Runner terminates its
process), its request waiting for a pair withdrawn from the broker and refused, its snapshot request
refused, a warning naming it. Its late completion is then discarded as a cancelled pipeline's, and
its invocation ends released or held by the fate its end reports. It is not recorded in
`cancelled_work`: the fold has already closed its identity, so no closure settles it.

## `impl Coordinator<'_>` › `fn judged(`

An attempt's completion, checked, then classified before anything else: an error is the interrupt at
once, every other pipeline cancelled, and only then is the pipeline retired (round R1: a fatal
completion interrupts as it is received, and nothing it held is granted on). A judgement is settled
by `settle_judged`, the function `step` settles through. A pipeline's error ends the command
(R-AB).

## `impl Coordinator<'_>` › `fn verified_arrived(`

A verification's completion, once checked: its review records charged to the run's spend at once, so
the next selection's ceiling check counts them (R-AH; the early review's `R1-CONC-4`), and its
outcome mapped at receipt by [`verified`]. An error — a caught panic, a Runner error the mapping
does not settle — is the interrupt there and then, with no wait for the snapshots (round R1,
`R1-CONC-2`); a result is held for `verify_concurrently`. One that no open verification awaits is a
cancelled pipeline's end (silent) or stale (warned).

## `impl Coordinator<'_>` › `fn cancel_all(&mut self) {`

Cancel every live pipeline's token, withdraw every pending registration, and refuse every reply
still owed — so no pipeline waits on a coordinator that is ending, and every one of them reaches its
completion. Registrations already granted are released as their pipelines end them, by the fate each
end reports. Each identity cancelled here is recorded, for a halt's closure to settle.

A pipeline inside an invocation is `Running` again, since its cancellation ends that invocation; one
waiting for a reply is `Running` only once its reply is refused here. A request the deterministic
intake has buffered but not applied is answered when it is applied, so its pipeline stays
`Awaiting`: marked `Running`, it would make the intake wait on the inbox for a pipeline that is
itself waiting on the intake (round R1's class search found the phase-3 shape doing that here, and
`stop` doing it first).

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

The invocations whose ends did not establish that their processes ended — reported unresolved, or
never reported: each keeps its registration and pair, and together they keep a halt's closure out.

## `struct Coordinator<'s>` › `abandoned: Option<SequenceId>,`

The sequence whose verification `verify` abandoned because the fold cancelled its transaction; read
once by `integrate`, which tells that from an error.

## `enum VerifyEnd {`

How `verify_concurrently`'s wait ended: its verification's result accepted and the attempt snapshots
drained, its transaction gone from the fold, or an interrupt.

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

## `mod tests` › `fn authorized_publication_completed_at_run_end() {`

Closure step (4) for T-FAST: the fast integration's CAS is made to fail once (`CasFailing`), which
leaves `merge_prepared(fast)` without `task_merged` — a state no coordinator schedule reaches at a
live end (R-AG) — and after a decline halts the run the closure publishes it before `run_finished`.
With `prepared_publication_completed_at_run_end` (T-PREPARED: a stale-clean verified proposal,
built on the coordinator so that beta is dispatched beside alpha and integrates stale; the answer
source is armed only once the publication is pending) and
`promoting_completed_by_the_closure_at_run_end` (T-CAND-REF, closure step (3): the candidates ref
made to fail once). The frozen width-1 `promoting_completed_at_run_end` (`candidate/tests.rs`) tests
the completion functions themselves.

## `mod tests` › `fn retained_generation_closed_at_run_end_at_width_three() {`

Closure step (5) for T-RETAINED at both endings: at Halted, beside an interrupted pipeline (the
decline is armed by the retained settlement's own append, `ArmsOnRetained`, so the halt lands at the
admission pass that would have admitted the retry); at BudgetExceeded, where the ceiling refuses the
retry the retained generation was ready for.

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
entered. Since round R1 the unresolved fate reaches the coordinator in the worker's `Ended` too, and
the worker keeps its registration.

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

A pipeline that fails, with `a_panicking_pipeline_ends_the_command_with_a_defined_error`. Since round
R1 each asserts the ledger with `settled_but`: the gate whose process was unresolved, and the gate
the panic unwound through, keep their registrations, and every other invocation is settled once.

## `mod tests` › `fn settled_but(run: &mut TopologyRun, held: &[InvocationId]) -> Result<(), String> {`

The ledger after the command ended: every registration settled exactly once but the ones in `held`,
whose processes the Runner could not establish as ended. Those stay running and keep their slot
pairs for the rest of this process — R3 and R4 release them at process death — and nothing is
pending and no settlement was a duplicate. This is what "the ledgers balance" means once an
invocation is held (round R1, `R1-CONC-1`).

## `mod tests` › `fn an_unresolved_end_keeps_its_pair_and_no_waiting_invocation_starts_on_it() {`

The early review's `R1-CONC-1` (the working record's §13, round R1). At width two with one slot for
the agent, alpha's worker holds the pair and beta's waits for it. Alpha's process ends unresolved —
the Runner could not establish that it is gone — and alpha reports that end before its error
completion. The phase-3 `Ended` carried no fate, so the coordinator released the pair at once and
granted it to beta, whose process started while alpha's might still run. Now beta's worker never
starts, the command ends with alpha's own error, and alpha's worker keeps its registration and pair.

## `mod tests` › `fn an_unreported_end_keeps_its_pair_and_no_waiting_invocation_starts_on_it() {`

`R1-CONC-1`'s other release: the double panics on alpha's thread as alpha's worker ends
(`RecordingRunner::panic_when_released`), so alpha never reports the end and its caught panic is its
completion. `retire` used to withdraw the invocation alpha still held and grant its pair to beta.

## `mod tests` › `fn a_fatal_verification_completion_interrupts_on_receipt_without_waiting_for_the_snapshots() {`

The early review's `R1-CONC-2`. Gamma holds a review snapshot, granted inside the verification,
when the verification's body panics (`WidePlans::panic_verifying`); the caught panic is the
verification's completion. The phase-3 coordinator kept it in `arrived` until every attempt snapshot
had drained, so gamma was not cancelled, ran its review to the end and settled first. Now the
coordinator is never asked, with that completion received, to wait for the drain; gamma's held
review is cancelled; nothing is appended after `merge_verification_started`.

## `mod tests` › `fn sibling_parked_on(`

The sibling repair of `root` that the fold's own sibling-repair tests build
(`topology/fold/tests/questions.rs`): spawned, dispatched and started, then parked on an embedded
question — four events a test plants through `Release::Append`, because the live engine of this build
spawns a repair only from a rejection and never runs two members of one lineage at once.

## `mod tests` › `fn a_declined_embedded_question_stops_the_verification_its_lineage_had_started() {`

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

## `mod tests` › `fn a_declined_embedded_question_stops_a_running_sibling_attempt() {`

`R1-CONC-3`'s attempt side, in the state the fold's
`declining_an_embedded_question_closes_a_running_sibling_and_refuses_its_late_result` builds: beta's
worker is held when the decline closes beta's in-flight generation. The phase-3 coordinator let
beta's pipeline run on — its gate was registered and run for a generation the fold had closed — and
only discarded its completion. Now beta's worker is cancelled and nothing more of beta starts.

## `mod tests` › `fn bounded(what: &'static str, body: impl FnOnce() + Send + 'static) {`

Run a scenario on a thread named after the test's, and wait for it a bounded time: the regression the
two tests below guard against is a coordinator that waits for ever, and it must fail the test, not
hang the suite. A scenario that never ends leaves only its own fixture's thread blocked.

## `mod tests` › `fn a_halt_answers_a_request_its_intake_still_buffers() {`

Round R1's class search (the working record's §13): beta reports its worker's end and asks for its gate
snapshot before the deterministic intake applies either, and the pass after the end ingests a halting
decline, so `cancel_all` meets beta with its request still buffered. The phase-3 transition marked
every cancelled pipeline `Running`, and the intake then waited on the inbox for beta, which was waiting
on it. Now the buffered request is applied, refused, and beta's attempt is settled interrupted by the
closure.

## `mod tests` › `fn a_stop_answers_a_request_its_intake_still_buffers() {`

The same position through `stop`: the verification reports its gate's end and asks for its review
pair before either is applied, and the pass after the end ingests the decline that fails its lineage.
Without the reconcile the run is stuck; with the reconcile and the phase-3 transition it waits for
ever; as repaired the review request is refused and never starts, and the run completes.

## `mod tests` › `struct DecliningAtPoll(std::sync::Arc<std::sync::atomic::AtomicUsize>);`

An answer source that declines at the n-th poll after it is armed with n: each admission pass polls
an open question once, so a scheduler that arms it with 2 as it releases a pipeline's last process
places the decline in the pass after that pipeline's completion is applied, the end having been
applied in the pass before.

## `mod tests` › `fn a_halt_after_the_verifications_result_arrived_interrupts_it_and_publishes_nothing() {`

Review round 2's `R2-CONC4-1`. Gamma is parked on a question and alpha has merged; beta's
verification is the only live pipeline, and its last process, a review, is released with the decline
armed for the pass after the verification's completion. That completion is accepted — nothing is
discarded — and its pipeline retired, so the halt is ingested with no pipeline live and the
transaction still `VerificationStarted`. The phase-4 guard read the live table and did not act:
`verify` returned the result and `integrate()` appended `merge_prepared` and `task_merged` after
`question_answered`. Now the halt interrupts `verify`, and the closure appends
`merge_verification_interrupted`, deletes the pin and reclaims the staging and the sequence's
snapshots before `run_finished`.

## `mod tests` › `fn a_decline_that_cancels_the_open_verification_goes_on_to_integrate_the_queued_candidate() {`

Review round 2's `R2-DECLINE-QUEUED`: round R1's verification witness with gamma's candidate queued
before the decline. Alpha merges, which lets delta dispatch; beta's candidate is under verification
with its gate held; gamma runs to its candidate inside `verify`; a sibling repair of beta parks; and
delta's worker ending opens the pass that ingests the decline. The phase-4 coordinator stopped the
verification and went on selecting in the same pass, found gamma's integration eligible with the
transaction gone, and refused it inside `verify` — the command ended. Now the pass ends, `verify`
waits for its cancelled pipeline and unwinds, and gamma's candidate is integrated next.

## `mod tests` › `fn with_two_closed_siblings<T>(`

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

## `mod tests` › `fn stopping_two_closed_pipelines_grants_neither_the_pair_the_first_one_frees() {`

`R2-RECONCILE-GRANT`: `reconcile` stops the root first, and withdrawing its request frees the pool the
sibling waits for. The phase-4 coordinator sent the sibling the grant before it stopped the sibling,
whose process could then start for a closed identity. Now the grant is withdrawn undelivered, both
requests are refused and settled cancelled, and the holder keeps its pair.

## `mod tests` › `fn a_pipeline_whose_identity_closed_is_granted_nothing_before_it_is_stopped() {`

`R2-RECONCILE-GRANT`'s class: every other place a grant is delivered, reached in the step between an
append that closes an identity and the admission pass that stops its pipeline — the order the
production intake keeps and an observer's `Release::Append` does not. The closed sibling's gate
registration, its snapshot request, its queued snapshot when the gate opens, and the pair the holder's
end frees (granted to the closed root, then, withdrawn, to the closed sibling) are all refused; the
phase-4 coordinator granted every one of them.

## `mod tests` › `fn every_container_invocation_is_launched_and_released_on_its_own_at_width_three() {`

R19 and R26 under concurrency (phase 5, R-AP). A width-3 run whose pipelines run the production
`ContainerRunner` over the shared fake, each container's process being the scaffold double's
invocation (`scaffold::Contained`), so the scheduler holds and releases containers as it holds and
releases invocations. At every quiescent point the physical inventory — the containers running in
the runtime, the intents under `<R>/containers`, the views under `<R>/views` — is exactly the names
of the invocations inside the runner; at the end all three are empty. Every process the double ran
had one container, created once and removed once, and each mounts its agent's credential volume
exactly when its role is given one. Three containers ran at once.

## `mod tests` › `fn container_coordinator_child() {`

The second coordinator of the two-process tests (R-AM): this test binary run again, `--exact` and
`--ignored`, by `scaffold::Served`, with its stdio linked to the parent (`ParentSide`) and its
container runtime the parent's (`LinkedRuntime`). Its role is named by the environment:
`fresh` creates a durable width-3 run under a ULID-shaped incarnation, reports its fixture root and
runs its coordinator until it is killed (`run_held`: every container holds, since nothing in the
parent completes a child's container); `resume` adopts a root, resumes it through the frozen
recovery order over the parent's runtime and the production `LockProbe`, with a pre-flight whose
shell probe is a container, reports what recovery settled and runs until killed; `census` is a
fresh write command in a second repository — a read-only pre-lock refusal first (an unknown run id),
then the worktree lock, then `startup_census` — and reports the census, then, when asked, takes its
own run lock and runs one agent-probe container that mounts the shared credential volume; `owner`
holds a run lock of its own and two probe containers until it is killed, which is how a dead owner
comes to exist. `hosted` (Unix) is a coordinator whose pipelines run real host processes
(`HostHeld`).

## `mod tests` › `fn holds_nothing(run_dir: &std::path::Path, repo: &std::path::Path) -> Result<(), String> {`

R17, observed (R-AQ): nobody holds the run — `rundir::is_running` answers from this process's
claims, then from the OS — and the repository's worktree lock can be taken, and is let go at once.

## `mod tests` › `fn broker_is_empty(run: &mut TopologyRun) -> Result<(), String> {`

R3, R4 and R13 at a coordinator's start: nothing registered, running or pending, no pair held or
waited for, no entitlement held, no reservation ever taken or cancelled, no duplicate counted.

## `mod tests` › `fn a_foreign_census_reclaims_a_dead_coordinators_containers_and_leaves_a_live_coordinators_running()`

ST-16 (b) under concurrency, in three processes. A dead owner — a child holding its run lock and two
containers, killed — leaves its containers running in the shared runtime and its lock free. The live
coordinator A, this process, runs three pipelines' containers; at its first quiescent point with all
three inside the runner, a foreign write command in a second repository under the same private root
(R-AN) runs its census as a child, through the parent's daemon. The census reclaims exactly the dead
owner's containers (`foreign-run-dead-owner`) and classifies A's as a live owner's
(`foreign-run-live-owner`), probing A's `run.lock` across the process boundary; A's three run on
across it, and every removal of them in the journal is A's own. The foreign command's first
invocation mounts the shared credential volume, and the journal puts it after the census observed
every dead container terminated and removed it. R17: the owner's lock went with its process, A's is
held until A ends and then nobody's, and the foreign command holds nothing after it exits; its
pre-lock refusal left nothing held (it took the worktree lock right after, in the same process).

## `mod tests` › `struct LedgerWatch<'p> {`

A container observer for the census of a resume, recording the pre-flight's ledger — its
settlements and its running registrations — at every container site the census passes. With
`CensusWatch` it is the recovery order's hooks.

## `mod tests` › `fn a_resuming_incarnation_reclaims_its_earlier_incarnations_containers_before_its_ledgers_probes_and_admission()`

ST-16 (f) and R-AO. Incarnation 1, a fresh child, is killed with three pipelines' containers
running; incarnation 2, a resuming child, reclaims those three in its census and is killed while its
own shell probe's container runs. Incarnation 3, this process, resumes: its census reclaims
incarnation 2's probe — the same deterministic `InvocationId` as its own shell probe, under another
container name and intent path — and the journal puts every census operation on it before
incarnation 3's first container operation. At every reclaim site the census passed, the pre-flight's
ledger held nothing; the probe ran after it and settled; the broker was empty when it was built.
The three containers incarnation 3 starts afterwards at width three are a live owner's to a foreign
census run as a child at its first quiescent point with three inside the runner, which reclaims
nothing and leaves them running. Each dead incarnation's holds went with its process.

## `mod tests` › `fn crashes_across_three_incarnations_with_pipelines_in_flight_leave_every_orphan_reclaimed_and_no_name_twice()`

ST-16 (g). Incarnation 1 is killed with three pipelines in flight; incarnation 2 resumes as a child,
reclaims incarnation 1's three containers, settles its three attempts interrupted, passes its
pre-flight (probes exit) and is killed with three pipelines of its own in flight; incarnation 3, this
process, reclaims those three, settles their attempts and completes. Across the three incarnations no
container name and no intent path occurs twice, and wherever two incarnations ran one
`InvocationId` — the shell probe, in incarnations 2 and 3 — their names differ. Six
`attempt_interrupted` and two `run_resumed` in the log; nothing left under the private root.

## `mod tests` › `fn a_foreign_census_and_a_resuming_incarnation_converge_on_one_dead_container_as_two_processes()`

ST-16 (h), as two processes. A dead owner's two containers are under the private root; the fake paces
the first at `Stop` and `Remove` for two parties, so the resuming incarnation (this process's
recovery order) and a foreign write command (a child in a second repository) have both classified it
before either kills it, and both observed it terminated before either removes it. Both reclaim it —
each actor's `Stop` and `Remove` are in the journal — neither refuses, and the root converges clean.

## `mod tests` › `fn a_surviving_reaper_hold_refuses_the_next_coordinator_until_released_and_is_never_reset_at_width_three()`

R28 at width three (Unix; R-AQ, R-Z). A width-3 run is shut down with its three workers in flight;
each pipeline's Runner call carried exactly the run's cleanup lease path (`CarriedLeases` records the
calls). A simulated surviving reaper — `rundir::tests::cleanup_hold_child`, a real process holding
the shared lock — then holds R28: the next coordinator's recovery order is refused at its lock
acquisition, naming the hold, and the hold is exactly where it was; once the reaper lets go, the
resume proceeds with empty ledgers and completes.

## `mod tests` › `fn a_killed_coordinators_reapers_settle_its_pipelines_processes_under_r28_before_the_next_one_resumes_at_width_three()`

The OS matrix's Unix row at the coordinator, with real processes (R-AQ, R-AR). A coordinator child's
three pipelines each run a real host process through the production `HostRunner` (`HostHeld`: a
shell that records its pid and sleeps). While they run, the run's cleanup lease is held — by reapers
spawned from pipeline threads, which entered no scope and hold it through the paths the coordinator
carried (R-Z). The coordinator is killed; every one of the three processes is gone once its reaper
settles, the holds are released after them, nothing of the dead coordinator is held, and the next
coordinator resumes with empty ledgers and completes.

## `mod tests` › `fn the_broker_ledgers_balance_at_every_end_and_start_empty_at_every_next_start_at_width_three()`

ST-09's broker rows over the ends phase 4 built: Complete, Parked, Halted, BudgetExceeded, an append
error and a shutdown, each at width three. At each end nothing is registered and unsettled, pending,
held or reserved (`balanced_at_end`); at the next start the process holds nothing it did not take,
and a resumable end's broker starts empty (`broker_is_empty`), while a Complete or Halted run is
finalized and refused before any broker is built. The killed coordinator's end is the process's
death, and its next start is asserted empty in the ST-16 (f) and (g) tests and the reaper test.
