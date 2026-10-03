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
early review's repairs (§13, round R1), review round 2's (§13, round R2) and review round 3's
(§13, round R3) corrected them; the sections below name them where they bind.

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
before the next selection (R-AH). The breach is seen at the next selection after a settlement
crosses the ceiling, and the pipelines live at that moment are not stopped: a budget stop drains
them, and each runs the rest of its body — the worker it may still be running, then its gates,
its review passes and their re-asks (a verification: its review passes and re-asks). So a run can
overrun its ceiling by the **remaining spend** of every pipeline admitted before the stop, at most
`max_parallel` of them, whatever that remaining work costs: not a count of invocations, since a
pipeline runs one invocation at a time and goes on starting them after the stop (round R5's
`FULL-SC-4`; the witness is `a_budget_stop_drains_live_pipelines_to_their_settlements_and_ends_budget_exceeded`,
whose two drained pipelines start their review passes after `budget_exceeded`). This exposure is
stated here only: the report carries no width and admits no field this slice would add, and it is
never a durable field ("durable_events: none new"); `PR11-OVERSHOOT-BOUND-IN-NO-OUTPUT` carries it
to an output.

### The run's end (phase 4)

The closure under concurrency is the working record's R-AF..R-AK. A halt cancels every live
pipeline at once, waits for each to end, and hands the closure the identities it cancelled
(`closure::Cancelled`); the closure settles exactly those interrupted and completes promotions and
authorized publications. A budget stop admits nothing and drains: live pipelines settle as they
would have, and the closure runs once none is live; a halting settlement drained while others are
live makes the next admission pass cancel them, so the closure runs as Halted. A cancelled
pipeline whose Runner could not establish that its process ended keeps the closure out: the
command ends resumably and names it. A shutdown cancels and drains the same way and appends
nothing. An append error runs the protocol in `emit.rs` and ends the command with its report; the
protocol withdraws the waiting requests, and each running invocation is released by its own
pipeline's report of its end once the Runner has established that its process is gone — one whose
process is unresolved keeps its registration and its pair, as at any other end (the working
record's R-AI, as corrected in round R5: the coordinator's broker is built `for_pipelines`, and its
ledger settles no running registration at the protocol).

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
- **Every admission pass stops each pipeline whose identity the fold has closed**, before it selects
  anything — a decline that does not halt fails its lineage, and so does a lineage member's failed
  settlement — and `verify` unwinds when its transaction disappears, abandoning the integration
  without ending the command (`R1-CONC-3`). A halt the fold records is acted on before that (round
  R3, below).
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
  the fold, and no interrupt recorded (`receives`; round R3 adds: and no halt in the fold). A pair
  freed for any other — by the stop of another closed pipeline, by an end, by a withdrawn grant — is
  withdrawn undelivered and its request refused, so no process starts for an identity the fold has
  closed; a registration or a snapshot such a pipeline asks for, or waits for, is refused the same way
  (`R2-RECONCILE-GRANT`).

### No grant once a halt is recorded, and a budget stop's drain (round R3)

Review round 3 (the working record's §13, round R3) found the admission pass reconciling before it
acted on a halt the fold had just recorded. Reconciliation withdraws the requests of the pipelines
whose identities the halting append closed; a withdrawal at the head of the queue frees the pair an
unrelated, open pipeline waits for, and `reply_granted` sent it that grant, because `receives` asked
only whether an interrupt was recorded, and the halt was in the fold and not yet in `interrupt`. The
open pipeline's process could start after the halt was durable, against the packet's "shutdown or
halt cancels pending requests" (`R3-HALT-GRANT`). As repaired:

- **The admission pass acts on a halt before it reconciles anything.** A halt the fold records, with
  anything in flight, is the interrupt at once: every live pipeline is cancelled and every pending
  request withdrawn while it is still pending (`cancel_all`), so no withdrawal frees a pair and the
  broker grants nothing; reconciliation then finds every pipeline cancelled.
- **No grant is delivered once an interrupt is decided or a halt is durable, whatever the order**
  (`interrupted`): `receives`, which every delivery of a pair or a snapshot asks, and
  `admit_invocation` refuse while an interrupt is recorded or the fold records a halt. A grant freed
  in the step between a halting append and the pass that acts on it — the step an observer's
  `Release::Append` opens — is withdrawn undelivered too. An append error needs no reading of its
  own: a poisoned fold holds no identity open (`Standing::of` reads `Holds::Nothing`), so both refuse
  through the identity check, and the error is the interrupt in the step it returns to. A shutdown and
  every other error are recorded in `interrupt` in the step that decides them.
- **A budget stop is not a halt, and the pipelines it drains are granted.** The closure procedure's
  step (2) drains a BudgetExceeded run "to natural settlements": each live pipeline runs its attempt
  or verification to the terminal it would have reached, and every invocation it has still to run
  needs its pair (R-AH: "slot and snapshot grants go on so live pipelines can finish"). So a pending
  request of a live pipeline whose identity is open is still granted under a budget stop — by an
  end, a withdrawal or a reconciliation — as before the stop. What the stop ends is admission of new
  work (step (1)), never the invocations of work already admitted. A halting settlement drained
  under the stop is a halt from that append on.

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

`Wake` is the coordinator's own: the timer of one wait of a registry access the coordinator makes
sends it, with that wait's token, into the coordinator's inbox (`registry_pause`; the follow-up B
record's §9.13, R1). It names no pipeline. A wake that arrives outside its wait is stale and is
dropped.

## `impl ToCoordinator` › `const fn completes(&self) -> bool {`

A completion — `Judged` or `Verified` — which a registry access's wait defers until the transition
it waits in returns, because applying one appends.

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
an interrupt, why the unbounded channels are bounded, and how a registry access the coordinator
makes waits only for messages.

## `struct Coordinator<'s>` › `deferred: VecDeque<(Origin, ToCoordinator)>,`

Completions a registry access's wait received and deferred, applied first by [`Self::next_message`]
once the transition returns.

## `struct Coordinator<'s>` › `wakes: u64,`

The last wait's token; each wait takes the next, so a stale wake never ends a later wait.

## `struct Coordinator<'s>` › `ledger: crate::util::DurabilityLedger,`

The run's hooks' durability ledger, taken once when the coordinator is built, for the manager's
funnels that take the coordinator as their hooks: `durability_ledger` reads through `&self`, and the
run's hooks are reachable only mutably.

## `struct Coordinator<'s>` › `refusal: Option<String>,`

Why the run's hooks answered the last phase they refused, read right after the answer, for the same
reason.

## `impl Coordinator<'_>` › `fn drive(&mut self) -> Result<Progress, UpstrokeError> {`

Admit everything selection admits; end the command on an interrupt; when nothing is live, take the
idle-only arms; otherwise apply the next message. An error anywhere is recorded as the interrupt
and the loop ends through [`Self::finish`].

## `impl Coordinator<'_>` › `fn admit(&mut self) -> Result<(), UpstrokeError> {`

Select and start until selection has nothing to start now. Each pass reads the fold afresh
(INV-21): a poisoned fold refuses; a halt the fold records with anything in flight (`any_in_flight`)
is acted on first — every pipeline cancelled, every pending request withdrawn — before anything is
reconciled, so no withdrawal can grant a pair after the halt (round R3, `R3-HALT-GRANT`); every
pipeline whose identity the fold no longer holds open is stopped (`reconcile`); an ending run stops
admission, and a budget stop lets it drain (R-AC, R-AH); while
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

## `impl Coordinator<'_>` › `fn answer_until_woken(&mut self, token: u64) -> Result<(), UpstrokeError> {`

One wait of a registry access the coordinator makes (R1): answer messages until the wake with
`token` arrives. Without an observer, each message in arrival order. With one, the deterministic
intake's own steps — an injected message, everything arrived buffered, a wait while a pipeline is
`Running`, the buffered message that sorts first — over the messages the wait may apply, and the
observer at quiescence; when the observer releases nothing, the wait waits for its next message,
since the wake is on its way. It returns at the wake, leaving later messages where they are.

## `impl Coordinator<'_>` › `fn answer_paused(&mut self, origin: Origin, message: ToCoordinator) -> Result<(), UpstrokeError> {`

What a wait does with a message: a grant request, an end, a snapshot request or end, and a shutdown
are applied as [`Self::handle`] applies them — none appends — and a completion is deferred; a stale
wake is dropped.

## `impl Coordinator<'_>` › `fn buffer_paused(&mut self, message: ToCoordinator) {`

Buffer a message for the observer's intake, dropping a stale wake.

## `impl Coordinator<'_>` › `fn take_canonical_answerable(&mut self) -> Option<ToCoordinator> {`

The buffered message that sorts first by pipeline and arrival among those a wait may apply.

## `impl Coordinator<'_>` › `fn observe_paused(&mut self) -> Result<bool, UpstrokeError> {`

The observer at a wait's quiescence: a released invocation's pipeline is `Running` again, and
nothing released only means the wait goes on. An append asked for inside a wait is refused: nothing
is appended inside another transition.

## `impl Coordinator<'_>` › `fn admit_invocation(`

A pipeline's `Admit`: refused and counted when injected, from a pipeline that is not live, or for
an invocation its identity does not own; refused as cancelled when its pipeline was cancelled, an
interrupt is recorded or the fold records a halt (`interrupted`, round R3), or the fold no longer
holds its identity open (round R2 — the broker checks the
standing of a slotted request, and a gate or the shell probe has none to check); otherwise
registered with the broker against the invocation's standing — granted at once (the reply is sent),
or pending until a slot pair frees (the reply waits in `replies`). A grant whose pipeline stopped
waiting is withdrawn, and whatever that frees is handed on by `reply_granted`.

## `impl Coordinator<'_>` › `fn interrupted(&self) -> bool {`

Whether the command is ending on an interrupt: one is recorded in `interrupt` — a halt acted on, a
shutdown, an error — or the fold records a halt the admission pass has not acted on yet. A budget
stop is neither. The phase-4 checks read `interrupt` alone, and a halt is recorded there only by the
admission pass that acts on it, so between the halting append and that pass a grant could reach an
open pipeline (round R3, `R3-HALT-GRANT`).

## `impl Coordinator<'_>` › `fn receives(&self, pipeline: PipelineId) -> bool {`

Whether `pipeline` may be handed a grant — a slot pair or a snapshot: it is live, not cancelled, the
fold holds its identity open, and the command is not ending on an interrupt (`interrupted`: none
recorded and no halt in the fold). Every place a grant is delivered asks it (round R2,
`R2-RECONCILE-GRANT`; the halt in the fold, round R3).

## `impl Coordinator<'_>` › `fn reply_granted(&mut self, granted: Vec<InvocationId>) {`

Hand on what a settlement granted. The broker grants by its queue alone and knows nothing of the
fold, so each waiting pipeline is asked `receives` first: one that does is sent the grant; any other
is refused, its grant withdrawn from the broker undelivered — no process of it started — and whatever
that frees handed on the same way, with a warning. The phase-3 shape sent every grant: stopping one
closed pipeline withdrew its request, which, at the head of the queue, had reserved a pool another
closed pipeline's request waited for, and the pair went to that pipeline before it was stopped
(round R2). And round R2's shape sent it to an open pipeline after the fold had recorded a halt, the
pass not yet having acted on it (round R3).

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
(`receives`: not live, cancelled, its identity closed, or an interrupt recorded or a halt in the
fold); granted at once when
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
so none of them starts a process (round R2, `R2-RECONCILE-GRANT`). A halt recorded by the same append
is acted on before this runs (round R3): the halt has cancelled every pipeline and withdrawn every
pending request, so under a halt nothing is left here to stop and no withdrawal frees a pair.

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
completion. Pending requests are withdrawn all at once, never one by one, so none of them frees a pair
or moves the queue's head for another: nothing is granted on the way. Registrations already granted
are released as their pipelines end them, by the fate each end reports. Each identity cancelled here
is recorded, for a halt's closure to settle; since a halt is acted on before reconciliation (round R3),
that can include a pipeline whose identity the halting append itself closed, which settles nothing —
the closure settles only what the fold shows in flight.

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

## `impl Operator for Coordinator<'_> {`

The coordinator runs its transitions with itself as the operator: the run, its seams and its own
hooks for the transition's steps, and itself as the registry hooks
(`run::begin_dispatch`, `run::begin_retry`, `run::settle_judged`, and through `DrivenJournal` the
frozen `integrate()`).

## `impl TopologyHooks for Coordinator<'_> {`

The hooks the coordinator lends a registry access: its own effect hooks (`effects` is the
coordinator, below), and the run's for the rest. `spawn` is called by path because the process-start
census (`runner::contract`) counts the text `.spawn()`, and this accessor starts nothing. No
`folded`: no append runs through these hooks — every append goes through the run with the run's own
hooks — and a frozen census (`events::log::tests`) lists the production modules that name the
fold's type, which this one does not.

## `impl crate::workspace_manager::EffectHooks for Coordinator<'_> {`

The run's effect hooks, with the wait of a registry access answered here. `phase` forwards and keeps
the refusal's cause for `refusal_cause`; `durability_ledger` is the run's, taken at construction.
`registry_pause` starts a timer thread that sleeps the wait's length and sends the wait's `Wake`,
answers messages until that wake ([`Self::answer_until_woken`]), and joins the timer; a timer that
cannot be started is reported and the wait is slept, as before.

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

## `mod tests` › `fn scheduled_here(every: std::ops::Range<u64>, on_windows: &[u64]) -> Vec<u64> {`

The schedules a seeded test runs on this platform (the record's R-AY): every seed of `every` on Linux
and macOS; on Windows only `on_windows`, which must be a non-empty subset of them. A seed is one
schedule of the coordinator's logic, which has no platform branch and is reproduced exactly on every
machine (R-AD); what Windows adds is its own I/O — process creation, Git for Windows, its paths —
which every schedule exercises alike, and there a seeded test costs 37 to 44 times what it costs run
alone on Linux (the CI guest's reconstructed durations; each schedule starts two hundred to three
hundred Git processes, nearly all of them the manager's Git effects). So Windows runs the fewest schedules a test's assertions compare: one per
configuration, two where a test asserts equality across seeds, and for a claim made of *some* seed
the seed that makes it (the Linux runs name it). Tests whose behaviour differs on Windows — the kill
matrices, the two-process tests, the container runner, the scripted shapes — run in full everywhere.

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
others are in flight, and the resume following the surviving prefix (R-AI). Since round R5 the
ledger it asserts has no duplicate settlement: each of the two running registrations is settled once,
by its own pipeline's end report after its process terminated, and not by the protocol before.

## `mod tests` › `fn an_append_error_releases_a_running_invocation_only_when_its_end_establishes_its_process_gone()`

The full review's `FULL-CONC-1` (its concurrency lens's witness): the same append error with beta's
cancelled process reported unresolved (`RecordingRunner::unresolved_when_cancelled`). Beta keeps its
registration and its pair, gamma is released by its end, nothing is settled twice, and the ledger
does not balance — it reports no release and no balance it has not observed. At `a9e88039` the
protocol had settled both before either pipeline ended: running none, holders none, balanced, two
duplicates.

## `mod tests` › `fn kill_inside_closure_recovers_at_width_three() {`

T-FINISH at width three, with
`append_error_inside_closure_ends_command_and_resume_completes_closure_at_width_three`: `HaltArming`
arms the fault at the first append after the halt is folded, which is the closure's first terminal;
the child (`closure_kill_child_at_width_three`) dies inside it, torn or complete, and the next
process repeats the closure from the surviving prefix. Each child's temporary directory is a `kill_dir`
guard this test holds (`scaffold::child_temporary_of`), so the fixture the child builds and the neutral Git
configuration it writes are reclaimed with the guard rather than left in the suite's temporary directory.

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
after every append, and `ending_as` asks it what the fold would say to a `run_finished`. The drain
test also holds the overshoot's shape (round R5, `FULL-SC-4`): each pipeline live at the stop starts
its review pass after `budget_exceeded`, which `Ran::durable_at_spawn` shows, so the exposure is
every admitted pipeline's remaining work, not one invocation each.

## `mod tests` › `fn the_overshoot_notes_bound_the_remaining_spend_of_every_pipeline_live_at_the_stop() {`

Pins the `Budget overshoot` section above, as `run/tests.rs` pins its notes: it must state the
remaining spend, the review passes and re-asks, and that no count of invocations bounds it, and the
retired reading — one invocation per live pipeline — must not come back. Red against the notes at
`a9e88039`.

## `mod tests` › `fn run_finished_budget_exceeded_refused_after_halting_drain_settlement_at_width_three() {`

In this build a halting settlement is a declined answer, which the fold refuses while a budget stop
is current, so a halting drain settlement is planted on a width-three prefix (R-AH). It proves
**outcome precedence only**: the prefix is planted after a shutdown, with nothing live, so the halt
the planted settlement records has nothing to cancel; the fold derives Halted, refuses
`run_finished(BudgetExceeded)`, and the next coordinator's closure ends the run Halted. The
conversion of a live drain is the next test's (round R5, the full review's `FULL-SC-3`).

## `mod tests` › `fn a_halting_settlement_drained_after_a_budget_stop_cancels_the_live_worker_and_ends_halted_at_width_three()`

Closure step (2)'s conversion, live: alpha crosses the ceiling and `budget_exceeded` is recorded with
beta's and gamma's workers inside the Runner; the scheduler then drains a halting settlement of beta's
(`Release::Append`, as R-AH says it must be planted in this build) while gamma's worker is still
live. The next admission pass acts on the halt: gamma's worker is cancelled through the Runner, the
closure runs as Halted from step (2) and settles gamma's attempt `attempt_interrupted`, nothing is
published, and the run finishes Halted at beta. With the halt branch disabled during a budget drain,
gamma's worker runs on and the command fails on gamma's next snapshot request instead.

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
follows them. An injected completion meets every check a pipeline's does before the injector refuses
it as injected, so a count of discards cannot tell which check held; since round R5 (the full
review's `FULL-SC-2`) the test asserts which one refused each: the binding to the pipeline's identity
refuses alpha's two completions for another generation and another attempt, and the injector only
beta's own, which nothing earlier refuses. The binding and the fold's check are proved on the channel
a pipeline sends on by the three tests below.

## `mod tests` › `fn completion_on_the_pipeline_channel(`

Round R5's assembled coordinator for `FULL-SC-2`: alpha's and beta's attempts started at width
three, their pipelines live with their jobs, the fold optionally given a settlement of beta's attempt
first; the one message — a completion — is sent by the named pipeline's own `Client` into the
coordinator's inbox and taken by `receive_one` with no observer, the production intake, so it
arrives as `Origin::Pipeline` and meets `accepts` as a real pipeline's completion does. What the
coordinator did with it is returned whole (`Received`): whether it interrupted, which pipelines are
still live and cancelled, the discards counted and the events appended.

## `mod tests` › `fn a_pipelines_completion_naming_another_attempt_of_its_task_is_discarded_and_its_pipeline_kept()`

The binding, for a completion naming attempt 2 of the pipeline's own task: discarded and counted, by
the binding's warning, and the pipeline left live to send its own. With the binding removed the fold's
check refuses it instead and retires the live pipeline — whose own completion would then be stale.

## `mod tests` › `fn a_pipelines_completion_carrying_another_live_pipelines_identity_is_discarded_and_settles_nothing()`

The binding, for alpha's pipeline carrying beta's identity, which the fold holds open (the
concurrency lens's case): discarded and counted, nothing appended, both pipelines live. With the
binding removed nothing else refuses it: alpha's pipeline is retired and its job settled with a
completion that was not its own.

## `mod tests` › `fn a_pipelines_completion_for_an_identity_the_fold_closed_is_discarded_and_settles_nothing() {`

The fold's check: beta's attempt is settled first (a planted `attempt_finished`), and beta's pipeline
then reports its own completion before any pass has stopped it. Discarded, counted and retired,
nothing appended. With the check removed the completion is settled against an attempt the fold has
already settled, which the fold refuses and the command ends on.

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

## `mod tests` › `fn with_a_waiter_behind_a_decline<T>(`

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

## `mod tests` › `fn a_recorded_halt_withdraws_every_waiting_request_before_reconciliation_frees_a_pair() {`

`R3-HALT-GRANT` through the production path: the admission pass that follows the halting decline. It
acts on the halt before it reconciles, so every pipeline is cancelled, both waiting requests are
withdrawn while pending and refused, the broker grants nothing (its grant count is the holder's
alone before and after the pass) and nothing is handed out; no warning names a withdrawn grant, since
none was computed; the holder's, the waiter's and the asker's attempts, in flight, are vouched for the
closure; and only the holder's invocation runs, to be released by its own end. The round R2
coordinator reconciled first: withdrawing the root's request freed pool `p`, and the waiter — open,
uncancelled, with no interrupt yet recorded — was sent the grant, and could start its process after
the halt was durable.

## `mod tests` › `fn a_budget_stop_drains_so_the_pair_reconciliation_frees_reaches_an_open_waiter() {`

Its control, under a budget stop: the same pass records no interrupt, stops only the root, whose
identity the decline closed, and grants the waiter the pair the root's withdrawal frees — the drain of
a budget stop goes on granting live, open pipelines.

## `mod tests` › `fn grant_sites(`

Every other place a grant is delivered, reached in the step before the next admission pass — the step
an observer's `Release::Append` opens: the asker registers its worker for `{c}` (free, and clear of
the head's reservation), asks for a snapshot, waits for one while the gate is closed and is queued
when it opens, and the holder's end frees agent `a`, which the broker grants to the root at the head
of the queue and, that grant withdrawn from the closed root, to the waiter. It returns what each of
the asker's three requests was answered with and how many snapshots the gate holds.

## `mod tests` › `fn no_grant_reaches_an_open_pipeline_between_a_recorded_halt_and_the_pass_that_acts_on_it() {`

`R3-HALT-GRANT`'s class: with the halt recorded and not yet acted on, every one of those grants is
refused although every pipeline asking is open — the asker's registration (never registered), its
snapshot request, its queued snapshot (no snapshot held), and the pair the holder's end frees, to the
closed root and then to the open waiter; nothing is handed out, and nothing is pending or running. The
round R2 coordinator granted all of them but the root's.

## `mod tests` › `fn under_a_budget_stop_every_grant_site_still_grants_an_open_pipeline() {`

Its control: under a budget stop every one of those grants is delivered — the asker's registration,
its snapshot and its queued snapshot, and the freed pair to the waiter — and only the closed root is
refused.

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
claims, then from the OS — and the fixture checkout's worktree lock can be taken, and is let go at once.

## `mod tests` › `fn broker_is_empty(run: &mut TopologyRun) -> Result<(), String> {`

R3, R4 and R13 at a coordinator's start: nothing registered, running or pending, no pair held or
waited for, no entitlement held, no reservation ever taken or cancelled, no duplicate counted.

## `mod tests` › `fn served(`

A child coordinator (`container_coordinator_child`) served the parent's fake runtime
(`scaffold::Served`), its standard error kept as `<name>.stderr` in the test's `logs` directory — which is
also its temporary directory (`scaffold::child_temporary_of`). So a fresh child's fixture, which the
parent adopts once the child is dead, and the neutral Git configuration the child writes lie inside the
`logs` guard, which each test binds before its children and the runs it adopts from them, so they are
dropped first and the guard then reclaims the rest; the tests keep that directory's tag
short (`census-b`, `resume-f`, `crashes-g`, `converge-h`, `reapers`), since a fresh child's paths nest
inside it on Windows too (the record's phase-7 path arithmetic).

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
pre-lock refusal left nothing held (it took the worktree lock right after, in the same process). The
owner's release is read as a bounded wait (`within`, under `BOUND`), as `holds_nothing` reads the
others: `rundir::is_running` reads the run's cleanup lease once the run lock is free, and a sibling test
thread's fork can hold a copy of that lease for a moment after its last holder ends (the mechanism of
`PR281-CLEANUP-LEASE-HOLD-OUTLIVED-AND-ITS-UNREADABLE-TWIN`). A hold that outlives the bound fails the
test.

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
shell that records its pid and sleeps). The shell is handed the pid file's path as its argument, never
inside its program text (standards §9), and the pid directory's own name carries an apostrophe and a
space, so a path a shell would have to quote is exercised on every run: at `a9e88039`, which spliced
the path into the program in single quotes, no pid was recorded and the test failed at its bound
(round R5, the full review's `FULL-REG-1`). While they run, the run's cleanup lease is held — by reapers
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

## `mod tests` › `mod interleaving {`

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

## `mod interleaving` › `fn export(kind: &str, value: &serde_json::Value) {`

The G6 exporter. When `UPSTROKE_G6_EXPORT` names a directory, a test writes what it measured as
pretty JSON to `<dir>/<kind>/<test>.json`, `<test>` being the last segment of the calling thread's
name, which libtest sets to the test's path (the observation export keys on the same name,
`observations.md`). Kinds: `seam/<row>` (the seam-test
outputs, one file per test that claims the row), `projection/<plan>` (the permutation projection
equivalence report: per seed, each key's projection digest and the final tree), and
`ledgers/<ledger>` (`slot`, `invocation`, `provisional`, `entitlement`, `container`). Unset, nothing
is written. The record's §3 R-AU gives the gate's command.

## `mod interleaving` › `struct Interleaver<'r> {`

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

## `mod interleaving` › `fn aliases(`

ST-04's oracle over one whole run, from the durable log and the double's record of every process:
every `InvocationId` started once; every task worktree and snapshot path used by exactly one owner
(a task's generation for its worktree; an attempt's gates or a reviewer's pass and re-ask, or a
sequence's integration gate or reviewer, for a snapshot) and every owner in exactly one path; each
generation dispatched once; each attempt started once and settled once (`candidate_prepared`,
`attempt_finished` or `attempt_interrupted`); each candidate ref created once and each prepared pin
taken once; each sequence claimed by one candidate, in log order; integration in
`task_candidate_created` order and each sequence merged once.

## `mod interleaving` › `fn duplicating(crossed: std::rc::Rc<std::cell::Cell<usize>>) -> Inject<'static> {`

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

## `mod interleaving` › `struct Tracing {`

The coordinator's hooks for ST-13, ST-10 and ST-18: the harness adapters, with the effect and
run-directory families wrapped to record every consultation, in order, into one trace beside every
append (its kind, the fold-derived pipeline and merge holdings after it, which reservation kind it
converts, whether it is `merge_prepared(fast)`), and every fold state for the replay check. Only the
coordinator thread's effects are traced: each pipeline takes its own hooks from the factory.
`Killer` counts the consultations after `run_finished` is folded and answers `Kill` at the armed one,
through the observation export as the harness adapters do, having written the cell it fires at to a
report file first — which is how ST-18's parent learns where its child died.

## `mod interleaving` › `fn provisional_problems(`

ST-13's oracle over the trace. At every append the derived holdings are within `max_parallel` and one
merge; each first append — `task_dispatched`, a retry's `attempt_started`,
`merge_verification_started`, `merge_prepared(fast)` — moves them by exactly its reservation's
entitlements in one step (the fast and stale integrations' pair from no merge to one); no staging
effect (the staging intent and worktree, the prepared pin, the proposal cherry-pick) falls between a
fast integration's selection and its `merge_prepared`, or before its `task_merged`; the fast CAS's two
hook phases see the merge held; and `task_merged` releases both holdings at once. The reservation
ledger itself is read at every quiescent point and at the end, since the append hook does not carry it
(R-AT).

## `mod interleaving` › `fn seeded_runs_alias_no_task_generation_attempt_invocation_snapshot_ref_pin_sequence_or_queue_position()`

ST-04 over eight seeded runs of a four-task plan with a dependent, a same-generation retry and two
reviewers per attempt and per verification, with ST-15 (the retry re-gates on a fresh snapshot of its
own in every seed, and runs beside another task's process in some — in all eight, so seed 0 is the
Windows schedule).

## `mod interleaving` › `fn injected_duplicates_at_seeded_points_release_nothing_twice_and_change_nothing_durable() {`

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

## `mod interleaving` › `fn every_provisional_reservation_converts_at_its_first_append_under_seeded_permutations() {`

ST-13 at every append of eight seeded runs (`provisional_problems`), no reservation outstanding at any
quiescent point, and at the end one conversion per first append with none cancelled or duplicated;
ST-10 on the same runs (the live fold equals a replay of its own log after every append).

## `mod interleaving` › `fn a_chain_projects_at_width_three_as_at_width_one_under_every_seed() {`

ST-08's chain half under eight seeds, each byte-identical to the width-1 loop's canonical projection.

## `mod interleaving` › `fn independent_tasks_project_identically_per_key_under_every_seed_and_report_it() {`

ST-08's independent half (and ST-03) under sixteen seeds: every task dispatched before any completion
on one base, integration in `task_candidate_created` order, and each key's attempt-level projection and
the final tree equal to the first seed's.

## `mod interleaving` › `fn reduced_limits_and_adverse_completion_orders_reach_run_finished_within_the_step_bound() {`

Deadlock-freedom: one slot per agent and per pool; two agents sharing one pool, each in its own, none
pooled, and one unpooled beside a pooled one; review re-asks taking pairs in two of the four; and each
under two seeds and three fixed adverse orders — on Windows each configuration under one of them, the
first seed, newest-granted-first, oldest-granted-first and gates-first in turn (`scheduled_here`), so
every configuration and every kind of order runs there once. Every run completes within `STEPS`, no point holds a
pair over a limit, every pair granted is released once, and a verification runs beside attempts. Each
run executes under the parent module's `bounded` watchdog on its own (`reduced_limit_run`), so a
regression that hung instead of ending in the stuck error fails the test rather than the suite, and the
bound is per run, not per test — a test of twenty runs on a slow leg does not approach it.

## `mod interleaving` › `fn a_scheduler_that_stops_releasing_ends_the_run_as_stuck_rather_than_hanging() {`

The bound's other half: a scheduler that stops releasing ends the run with the stuck error, every held
process cancelled and every registration settled, and nothing appended — under `bounded`, so a
coordinator that waited for ever instead fails the test within its bound.

## `mod interleaving` › `fn runtime_pool_same_agent_and_pool_with_opposing_limits_serialize_on_the_binding_limit() {`

Acceptance item 3 at runtime, with the four `runtime_pool_*` tests after it: the processes held at
every quiescent point are what runs at once, so a limit that binds shows as a ceiling on them and one
that does not as two at once. On Windows one seed per configuration; the gate test's is seed 1, one of
the two whose schedule runs two gates at once beside a held slot, the claim it makes of some seed.

## `mod interleaving` › `fn runtime_pool_agent_probes_take_and_release_their_pair_at_preflight_before_admission() {`

A width-3 run resumed through the frozen recovery order with a real `RunPreflight` over a double: the
shell probe, then one probe per recorded agent, each under its own probe-role `InvocationId`, each
settled once; the pre-flight's ledger balanced before admission; the coordinator's broker empty when it
starts, and the run completing on its pool.

## `mod interleaving` › `fn seeded_contained_runs_never_reuse_a_container_name_intent_or_view() {`

ST-04's container rows (names and intent paths) with R19 and R26 under seeds: at every point the
containers running, the intents and the views are exactly the held invocations', and no name is
created twice.

## `mod interleaving` › `fn a_halt_under_every_seed_interrupts_exactly_what_is_in_flight_and_ends_halted() {`

ST-17's halt under eight seeds: after the halting decline nothing is admitted, one interrupted terminal
is appended per attempt and verification in flight at it, and `run_finished(Halted)` ends the log.

## `mod interleaving` › `fn a_budget_stop_under_every_seed_drains_without_cancelling_and_ends_budget_exceeded() {`

ST-17's budget stop under six seeds: after `budget_exceeded` nothing is admitted and nothing is
cancelled, what was in flight settles naturally, and `run_finished(BudgetExceeded)` ends the log.

## `mod interleaving` › `fn finalization_kill_matrix(halted: bool) {`

ST-18 at width 3, one outcome per test —
`a_concurrent_complete_run_finalizes_through_the_frozen_finalization_and_converges_after_a_kill_at_every_cell`
and its Halted twin — so libtest runs the two matrices side by side rather than one after the other. A
Complete or a Halted run (seed 13 halts with two candidates queued) is finished once in this process,
which lists the effect and run-directory cells its finalization consults after `run_finished`; then,
for every cell, a child (`finalization_kill_child_at_width_three`) runs the same seed and dies by the
kill at that cell, and this process adopts its directory and resumes it twice through the frozen
recovery order. The first resume finalizes to the state the uninterrupted
finalization left (report outcome, retained candidates, refs under the run namespace, worktrees, the
execution root); the second finds the report current, removes nothing and refuses the same run; neither
appends, and planted answer files stay byte-identical. Every child's temporary directory is a guard the
cell holds (`scaffold::child_temporary_of`, tag `tmp`, short for the Windows path budget), declared before
the run it adopts, so the run is dropped first and the guard then reclaims what the child left.

## `mod tests` › `const REAPER_BOUND: Duration = Duration::from_secs(60);`

How long a witness waits for a dead coordinator's reaper to finish its calls. The reaper notices
the death within one of its 10-millisecond polls; the bound exists so a mutation that arms nothing
fails the test instead of hanging it.

## `mod tests` › `fn reaper_finished(`

Polls the relay until the reaper's calls are a listing, a kill and a removal per expected
container, and a final listing, or the bound passes.

## `mod tests` › `fn reclaimed_by_its_reaper(reclaimed: &Reclaimed<'_>) {`

One reaper reclaimed exactly the expected containers: its first and last calls are the scope's
listing argv, one `kill` and one `rm --force --volumes` per container, nothing else; delivered
to the fake, each was stopped and removed by the actor `reaper`, none is left, and no census
listed the runtime after the death.

## `mod tests` › `fn a_resume_killed_inside_its_pre_flight_probe(`

A durable run, then a resuming incarnation in a child over the parent's fake — the frozen
recovery order with a `RunPreflight` over a `ContainerRunner` — killed while its shell probe's
container runs. The relay is installed on the fake the child is served by, so the child's
runner arms over it with nothing passed in the environment.

## `mod tests` › `fn a_resuming_incarnations_pre_flight_probe_container_is_killed_by_its_reaper_when_the_coordinator_dies_inside_it()`

`R6-C2`: the incarnation's first container — a pre-flight probe inside the frozen `recover.rs` —
is covered; its reaper reclaims it before any census.

## `mod tests` › `fn the_reapers_scope_is_the_runners_identity_and_selects_its_first_probe() {`

`R7-D2`, two-process: the reaper's first listing carries exactly the probe's
`upstroke.private_root` and `upstroke.incarnation` labels as the daemon recorded them, and a
container of another incarnation under the same private root, running beside it, is neither
listed nor killed.

## `mod tests` › `fn stranded_child(parent: &crate::engine::topology::scaffold::ParentSide) {`

A width-three coordinator over the parent's fake that reports its error return, drops every
handle on its runner, reports that, and then waits for a file the parent writes before it exits
of its own accord, so the parent can observe it alive past its last handle.

## `mod tests` › `fn a_runner_whose_containers_are_unresolved_keeps_its_reaper_armed_past_its_last_handle_until_the_process_exits()`

`R6-C1`, end to end: the runtime cannot observe, stop or remove three running containers, so the
coordinator returns its error with their registrations held; while the child lives past its last
handle the relay has no call; when it exits, its reaper reclaims all three.

## `mod tests` › `fn a_contained_run_with_its_reaper_relayed(`

An in-process width-three run over a contained runner whose reaper's program is the relay, after
the relay's self-check (`R6-D2`): the stub, run by its path, records its call where the test
reads, so a reaper's calls in this process would be seen.

The relay is written and its self-check run by isolated children of the test binary
(`FakeRuntime::install_reaper_relay`, `FakeRuntime::run_reaper_relay`), each under a deadline,
so these controls make no host launch of their own (`FUA-I4-RELAY`, PR #328's implementation
review round 4): a host launch ends in a wait for its reaper's acknowledged exit that is
unbounded by design, and made here one stopped reaper held the test, and the suite with it. The
relay's other users in this module, `a_resume_killed_inside_its_pre_flight_probe` and the
stranded-runner test, install it the same way.

## `mod tests` › `fn a_coordinator_that_ends_disarms_its_reaper_and_kills_nothing_at_width_three() {`

`R6-D2`: a run that completes, its runner dropped: the relay holds only the self-check, and every
container was released by its own runner. Ending the reaper by end-of-file rather than `CANCEL`
(`fua-m10`) makes it list.

## `mod tests` › `fn a_runner_whose_containers_all_ended_disarms_its_reaper_and_the_relay_is_never_called() {`

`R6-C1`'s control: the observation is lost, so the pipelines fail, but every container is
established gone; the drop disarms and the relay is never called.

## `mod tests` › `fn every_container_an_incarnation_starts_is_covered_by_an_armed_reaper_with_its_scope() {`

The design property, in-process, through a resume's pre-flight (`resume_over` with a
`RunPreflight` over a container runner), `run_concurrently` at width three, and the width-1
`step`: every container start was covered by an armed reaper whose scope selects its labels, and
every container created was observed starting.


## `mod tests` › `struct TearsAForeignRegistration {`

PR11 review round 8's witness (`R8-CONC-1`), the verification face of
`PR11-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY`: a review-input policy that, at the
verification of a candidate (the staging worktree under `merge/`), plants another process's
registration half written in the shared store — `HEAD` and `gitdir` written, `commondir` opened
and empty, the state `git worktree add` passes through and leaves when it is killed there. Once:
the first verification only. With `finishes`, a thread plays the other process finishing its
write, but only after a registry access of this run has failed on it at least once — the
`CONTENDED_ATTEMPTS` handshake of `workspace_manager::tolerant_registry_access`'s contract
(`reviews/2026-10-01-pr11-follow-up-b-record.md` §6.4) — so the witness measures a retry and
never a store that happened to be whole. Through the fixture's primitives: a topology module
may name no `std::fs` write.

The planted `gitdir` spells its path as Git writes it (`workspace_manager::fixture::as_git_writes_it`,
the host runner's Git-for-Windows form with `/` separators, and the path's own bytes elsewhere), as
every registration #329's tests plant does (repair round 3, the record's §9.13, R6).

The policy owns its writer (repair round 3, R3). The verification that plants the tear stores a
`ForeignWriter` once. `finish` cancels it and joins it, and
hands back what it returned: `Ok` only when it finished the registration after an access failed on
it. `settle(false)` joins it without a cancel, so the join waits for the handshake or the writer's
watchdog. The writer looks for the handshake every millisecond until its `watchdog` (`BOUND`, as
`tearing` makes the policy); when the handshake never arrives it writes nothing and returns why.
The drop cancels and joins a writer no test finished, and fails with what that writer returned,
saying it on stderr instead when the thread already unwinds, so nothing the writer does lands
after the fixture is reclaimed. Before, the writer's handle was discarded: it wrote `commondir`
when its watchdog expired whatever had happened, and a writer that outlived its test recreated a
directory its fixture had already reclaimed.

## `mod tests` › `struct ForeignWriter {`

`TearsAForeignRegistration`'s writer: the sender that cancels it, and the handle `finish`, or the
policy's drop, joins.

## `mod tests` › `fn registry_drive(`

`run_concurrently` with the review-input policy replaced, and the first-released scheduler.

## `mod tests` › `fn tearing(`

The policy and the administrative directory it plants, named `name` in the run's own store.

## `mod tests` › `fn a_verification_beside_another_processs_registration_write_in_flight_spends_no_deferral() {`

The record's T2: a registration another process finishes writing never reaches the verification
as Git state. The verification's registry access fails on the write in flight, is attempted
again, and passes once the writer finishes; nothing is deferred and both candidates merge. The
policy's `finish` returns `Ok`: an access failed on the tear, and only then did the writer finish
it.
Before #329 the same interleaving was durable: one `merge_verification_unavailable` (Deferred),
or a park at `max_defers` (round 3's measurement of the round 8 witness).

## `mod tests` › `fn a_verification_beside_a_registration_that_stays_torn_ends_resumably_and_its_resume_reverifies()`

The record's T2′: a registration that stays torn — nobody finishes it — ends the command at the
access's deadline as `UpstrokeError::RegistryRefused`, naming the entry's `commondir`, with
nothing durable after `merge_verification_started`: no `merge_verification_unavailable`, no
question, no `run_finished`. The operator's remedy (remove the registration) and a resume settle
the open verification interrupted and verify the candidate again under a new sequence, and the
run completes with nothing deferred. Mutation m2 (the deadline's refusal typed `Git`) makes the
verification defer here.

## `mod tests` › `fn planted_alone(`

The tear as `TearsAForeignRegistration` plants it, with no run: a scratch tree of its own, the
registration's directory at `<root>/common/worktrees/foreign`, planted through `problem` at a
worktree path whose parent is `merge`, as a verification's snapshot is.

## `mod tests` › `fn a_foreign_writer_whose_handshake_never_arrives_writes_nothing_and_says_so() {`

Repair round 3's R3, the shape of the regular lens's watchdog witness: no access meets the tear,
so the writer's 20 ms watchdog expires. Joined without a cancel, it reports the watchdog, and
`commondir` is still the empty file planted. Before the repair the writer wrote `commondir` at
its watchdog anyway.

## `mod tests` › `fn a_foreign_writer_is_cancelled_and_joined_before_its_fixture_is_reclaimed() {`

R3, the shape of the regression lens's witness: `finish` cancels and joins the writer before the
fixture goes, and says it was cancelled, and the cancelled writer wrote nothing. The fixture is
then reclaimed, and the wake the writer waited for arrives late (`CONTENDED_ATTEMPTS` moved by
hand, as a late access would move it): nothing recreates the root. Before the repair the writer's
handle was discarded, and that late wake recreated the reclaimed directory.

## `mod tests` › `enum TearAt {`

Where the R1 witnesses tear a foreign registration (the follow-up B record's §9.13): when an event
is folded, or just before the n-th registry access the coordinator's thread starts after an event
is folded (`workspace_manager::fixture::before_registry_access`), which names an access no fold
precedes directly.

## `mod tests` › `struct Prober {`

The writer that finishes the tear, owned by `TearHeld`: the sender that cancels it and the handle
its `finish` or its drop joins (standards §10).

## `mod tests` › `enum Wakes {`

What lets the prober finish the tear: an invocation entering the runner (a pipeline was granted),
one leaving it (the observer released a held invocation inside the wait, and its end was applied),
or a registry access answering `Attempt` (the width-1 control, where no pipeline is there to serve).

## `mod tests` › `enum Torn {`

The two torn shapes a witness plants: an empty `commondir`, on which Git's enumeration dies, and a
registration that is `locked` and has no `gitdir`, which the removal scan cannot bind under
`WriterProof::Unknown` and the enumeration passes over. Each is a shape a killed `git worktree add`
leaves.

## `mod tests` › `struct Plant {`

The tear `TearHeld` plants in the run's own store — `HEAD`, a `gitdir` spelt as Git writes it and an
empty `commondir`, or `locked` alone — and the prober it starts, which completes the registration
once what it waits for happened after the tear and otherwise writes nothing.
## `mod tests` › `type FoldAct = (fn(&TopologyEventBody) -> bool, Box<dyn FnMut()>);`

An act `TearHeld` runs once when an event is folded, besides the tear: a broken worktree before a
retry, a file at a destination before an add.

## `mod tests` › `struct TearHeld {`

The run's hooks with one tear planted on the coordinator's thread at a `TearAt`, its prober owned
and joined, and an optional `FoldAct`. `finish` cancels and joins the prober and hands back what it
returned: `Ok` only when it finished the tear after what it waited for. The drop does the same for a
prober nobody finished. At `54a1ff14` the coordinator slept through every such access, so the prober
never saw anything and the access refused at its deadline.


## `mod tests` › `impl TearHeld` › `fn tearing(&mut self, torn: Torn) {`

The shape the tear takes.

## `mod tests` › `impl TearHeld` › `fn planted(&self) -> impl Fn() -> bool + 'static {`

Whether the tear has been planted, for a release order that waits for it.
## `mod tests` › `fn two_independent() -> [WideTask; 2] {`

Beta, key 0, then alpha, key 1: in the order the admission dispatches them.

## `mod tests` › `fn served_through(tag: &str, tasks: &[WideTask], at: TearAt) {`

The dispatch witness of R1, without an observer: the first task's pipeline is spawned and asks for
its worker while the coordinator dispatches the second, whose access meets the tear. The access is
passed only because the coordinator granted the worker while it waited.

## `mod tests` › `fn attempt_started_of(key: u32) -> fn(&TopologyEventBody) -> bool {`

The predicate of a task's `attempt_started`.

## `mod tests` › `fn task_dispatched_of(key: u32) -> fn(&TopologyEventBody) -> bool {`

The predicate of a task's `task_dispatched`.

## `mod tests` › `fn a_pipeline_is_granted_while_a_dispatchs_head_check_waits_on_a_torn_registration() {`

Census A1: the dispatch's head check (`integrate::dispatch_head_at`, H2), the first access of every
dispatch.

## `mod tests` › `fn a_pipeline_is_granted_while_a_dispatchs_revalidation_waits_on_a_torn_registration() {`

Census A2: the dispatch's own revalidation before `task_dispatched`.

## `mod tests` › `fn a_pipeline_is_granted_while_a_dispatchs_intent_waits_on_a_torn_registration() {`

Census A3, the implementation review's witness: the intent's revalidation after `task_dispatched`.

## `mod tests` › `fn a_pipeline_is_granted_while_a_dispatchs_add_gate_waits_on_a_torn_registration() {`

Census A4: the add's gate.

## `mod tests` › `fn a_pipeline_is_granted_while_a_dispatchs_add_waits_on_a_torn_registration() {`

Census A4: the add itself, whose waits run inside its funnel (`funnel_lending`).

## `mod tests` › `fn beta_settles_while_alpha_holds_its_first_gate(view: &Quiescent<'_>) -> Option<Release> {`

The release order of the observed witnesses: alpha's worker first, then beta's invocations while
beta is live, and alpha's held first gate only once beta is not — inside the wait of an access made
for beta, where alpha's second gate is then asked for and granted, and enters the runner.

## `mod tests` › `fn served_while_alpha_waits(tag: &str, failing: &[(u32, u32)], at: TearAt) -> Wide {`

`served_while_alpha_waits_with`, expecting the run to complete.

## `mod tests` › `fn served_while_alpha_waits_with(`

The observed witness for a settlement, an integration or a retry of beta while alpha holds its first
gate: two gates, so that alpha's next grant asks for no registry access first.

## `mod tests` › `fn candidate_prepared_of_beta(body: &TopologyEventBody) -> bool {`

Beta's `candidate_prepared`.

## `mod tests` › `fn candidate_created_of_beta(body: &TopologyEventBody) -> bool {`

Beta's `task_candidate_created`.

## `mod tests` › `fn merge_prepared_of_beta(body: &TopologyEventBody) -> bool {`

Beta's `merge_prepared`.

## `mod tests` › `fn retained_attempt_finished_of_beta(body: &TopologyEventBody) -> bool {`

Beta's `attempt_finished`.

## `mod tests` › `fn generation_closed_of_beta(body: &TopologyEventBody) -> bool {`

Beta's `generation_closed`.

## `mod tests` › `fn a_pipeline_is_served_while_a_settlements_path_read_waits_on_a_torn_registration() {`

Census D1: the promotion's changed paths.

## `mod tests` › `fn a_pipeline_is_served_while_a_settlements_parent_check_waits_on_a_torn_registration() {`

Census D2: the candidate's parent check.

## `mod tests` › `fn a_pipeline_is_served_while_a_settlements_tree_check_waits_on_a_torn_registration() {`

Census D2: the candidate's tree check.

## `mod tests` › `fn a_pipeline_is_served_while_a_settlements_worktree_removal_waits_on_a_torn_registration() {`

Census D3: the reclaim's worktree removal — its scan, met by a registration that is `locked` and has
no `gitdir`.
## `mod tests` › `fn a_pipeline_is_served_while_a_settlements_intent_removal_waits_on_a_torn_registration() {`

Census D3: the reclaim's intent removal.

## `mod tests` › `fn a_pipeline_is_served_while_an_integrations_decision_waits_on_a_torn_registration() {`

Census C1: `integrate::decide`'s publishability check (H1).

## `mod tests` › `fn a_pipeline_is_served_while_an_integrations_publication_waits_on_a_torn_registration() {`

Census C2: `integrate::publish`'s publishability check (H1).

## `mod tests` › `fn a_pipeline_is_served_while_a_retrys_worktree_check_waits_on_a_torn_registration() {`

Census B1: the retained retry's worktree verification.

## `mod tests` › `fn alpha_waits_for_the_tear(planted: impl Fn() -> bool + 'static) -> Script<'static> {`

The release order of the repair witnesses: as `beta_settles_while_alpha_holds_its_first_gate` once
the tear is planted, and before that alpha's worker and beta's invocations only, releasing nothing
otherwise, so a first revalidation that waits out its deadline does not spend alpha's held gate.

## `mod tests` › `fn served_while_a_repair_waits(`

The witness of a revalidation's repair arm (census A6, B1 and B2: the torn plan and its scans) and
of the verification's own read. With `residue`, a dead add's residue of a slot an intent names — its
intent written before the run, its registration with an empty `commondir` planted at the fold — makes
the first revalidation refuse at its deadline, the repair's plan find it, and its forced removal
remove it; the tear is planted at the access the test names. The residue is gone afterwards.

## `mod tests` › `fn a_pipeline_is_served_while_an_intent_removals_repair_plan_waits_on_a_torn_registration() {`

The reclaim's intent removal: its repair's plan (the torn plan, R-X alone) meets the tear.

## `mod tests` › `fn a_pipeline_is_served_while_an_intent_removals_repair_removal_waits_on_a_torn_registration() {`

The reclaim's intent removal: its repair's forced removal of the residue (the scan) meets the tear.

## `mod tests` › `fn a_pipeline_is_served_while_an_intent_removals_second_revalidation_waits_on_a_torn_registration()`

The reclaim's intent removal: the revalidation after its repair meets the tear.

## `mod tests` › `fn a_pipeline_is_served_while_a_verifications_repair_plan_waits_on_a_torn_registration() {`

The retained retry's verification: its repair's plan meets the tear.

## `mod tests` › `fn a_pipeline_is_served_while_a_verifications_second_revalidation_waits_on_a_torn_registration()`

The retained retry's verification: the revalidation after its repair meets the tear.

## `mod tests` › `fn a_pipeline_is_served_while_a_verifications_worktree_read_waits_on_a_torn_registration() {`

The retained retry's verification: its read of the worktree's record, inside its funnel, meets the
tear.

## `mod tests` › `fn closing_attempt_finished_of_beta(body: &TopologyEventBody) -> bool {`

Beta's `attempt_finished` that closes its generation.

## `mod tests` › `fn a_pipeline_is_served_while_a_closed_retrys_worktree_scrub_waits_on_a_torn_registration() {`

Census B2: a retry whose retained worktree was changed by hand closes, and its scrub's worktree
removal (the scan) meets the tear.

## `mod tests` › `fn a_pipeline_is_served_while_a_closed_retrys_intent_scrub_waits_on_a_torn_registration() {`

Census B2: the same scrub's intent removal meets the tear; the scan passes over an empty `commondir`.

## `mod tests` › `fn served_while_a_closed_retry_scrubs(tag: &str, torn: Torn) {`

The B2 witnesses: beta's first attempt fails and retains its worktree, a file is staged in it by
hand, the retry's verification fails, and the generation closes.
## `mod tests` › `fn a_pipeline_is_served_while_a_failed_settlements_worktree_scrub_waits_on_a_torn_registration()`

Census D4: beta's second failed attempt escalates and closes the generation, and its scrub's
worktree removal (the scan) meets the tear; the task then parks, as the scaffold's failing gates
leave it.

## `mod tests` › `fn a_pipeline_is_served_while_a_failed_settlements_intent_scrub_waits_on_a_torn_registration() {`

Census D4: the same scrub's intent removal meets the tear.

## `mod tests` › `fn served_while_a_failed_settlement_scrubs(tag: &str, torn: Torn) {`

The D4 witnesses: both of beta's attempts fail their first gate.
## `mod tests` › `fn alpha_waits_for_gammas_integration(view: &Quiescent<'_>) -> Option<Release> {`

The release order of the stale witnesses: everything but alpha's held worker while gamma is live,
and alpha's worker once gamma is not — inside the wait of an access of gamma's integration, where
alpha holds no snapshot, so the stale arm is not deferred (R-W).

## `mod tests` › `fn alpha_waits_for_gammas_merge(view: &Quiescent<'_>) -> Option<Release> {`

The same, holding alpha's worker until gamma's `task_merged`.

## `mod tests` › `fn served_while_a_stale_integration_waits(tag: &str, conflicting: bool, at: TearAt) {`

`served_while_a_stale_integration_waits_under` with `alpha_waits_for_gammas_integration`.

## `mod tests` › `fn served_while_a_stale_integration_waits_under(`

The stale witness: beta merges first, so gamma's integration is stale; with `conflicting`, gamma
writes beta's file, its pick conflicts, and the rejection's repair is dispatched. The tear takes the
shape `torn`. The prober wakes on an ending: alpha's released worker ends, and the coordinator
applies that end, inside the wait.
## `mod tests` › `fn candidate_created_of_gamma(body: &TopologyEventBody) -> bool {`

Gamma's `task_candidate_created`.

## `mod tests` › `fn task_merged_of_gamma(body: &TopologyEventBody) -> bool {`

Gamma's `task_merged`.

## `mod tests` › `fn merge_rejected_of_gamma(body: &TopologyEventBody) -> bool {`

Gamma's `merge_rejected`.

## `mod tests` › `fn repair_dispatched(body: &TopologyEventBody) -> bool {`

A repair's `task_dispatched`.

## `mod tests` › `fn a_pipeline_is_served_while_a_stale_integrations_intent_waits_on_a_torn_registration() {`

Census C4: the stale arm's staging intent.

## `mod tests` › `fn a_pipeline_is_served_while_a_stale_integrations_add_gate_waits_on_a_torn_registration() {`

Census C4: the staging add's gate.

## `mod tests` › `fn a_pipeline_is_served_while_a_stale_integrations_add_waits_on_a_torn_registration() {`

Census C4: the staging add.

## `mod tests` › `fn a_pipeline_is_served_while_a_stale_integrations_pick_waits_on_a_torn_registration() {`

Census C4: the cherry-pick's revalidation.

## `mod tests` › `fn a_pipeline_is_served_while_a_publications_staging_removal_waits_on_a_torn_registration() {`

Census C5: the publication's staging removal — its scan.

## `mod tests` › `fn a_pipeline_is_served_while_a_publications_staging_intent_removal_waits_on_a_torn_registration()`

Census C5: the publication's staging intent removal.
## `mod tests` › `fn a_pipeline_is_served_while_a_conflicts_classification_waits_on_a_torn_registration() {`

Census C3: `integrate_stale`'s `proposal_state` after the conflicting pick (H1).

## `mod tests` › `fn a_pipeline_is_served_while_a_rejections_staging_reclaim_waits_on_a_torn_registration() {`

Census C5: the rejection's staging reclaim — its scan.

## `mod tests` › `fn a_pipeline_is_served_while_a_rejections_staging_intent_reclaim_waits_on_a_torn_registration()`

Census C5: the rejection's staging intent removal.
## `mod tests` › `fn a_pipeline_is_served_while_a_repairs_materialization_waits_on_a_torn_registration() {`

Census A5: the repair dispatch's materialization.

## `mod tests` › `fn a_pipeline_is_granted_while_a_continued_dispatchs_worktree_check_waits_on_a_torn_registration()`

Census A6: the first process stops after beta's `task_dispatched`, at a file planted at beta's
destination; the next process resumes, starts alpha's attempt, and continues beta's open generation,
whose worktree check meets the tear.

## `mod tests` › `fn the_width_one_step_runs_the_same_transitions_and_its_access_waits_by_sleeping() {`

R-T's control: the width-1 `step` runs the same dispatch with its own hooks, whose waits sleep; the
tear is finished once the access has answered `Attempt`, and the run completes.

## `mod tests` › `struct RequiresAFailingFilter {`

A review-input policy that, at the first verification, makes every checkout of the repository
run a required smudge filter that fails (`info/attributes` and the repository's configuration,
through the fixture's `git`): a snapshot's checkout then fails after Git took its destination
over, as a checkout that cannot be made does.

## `mod tests` › `fn a_verification_whose_snapshot_checkout_fails_after_the_takeover_ends_resumably_and_its_resume_reverifies()`

The record's T15, verification face, and R14
(`PR329-A-GENUINE-CHECKOUT-FAILURE-AFTER-THE-TAKEOVER-REFUSES`): the judge's snapshot add fails
after the takeover, which nothing outside Git tells from a prune deleting the snapshot's
registration, so it refuses at once, resumably, and nothing durable is appended. With the
environment repaired, a resume settles the verification interrupted and verifies again; the run
completes. Before #329 the same failure was Git state, and the verification deferred and then
parked at `max_defers`; R14 is that narrowing, which needs the owner's disposition before G6.
Two tasks, because a single candidate already on the integration head merges with no
verification.

## `mod tests` › `struct DeniesTheSnapshots {`

Round 5's construction: at the first verification, the execution root's `snapshots/` is made
unwritable, so the judge's snapshot destination cannot be made.

## `mod tests` › `struct RestoresTheSnapshots {`

Restores `snapshots/` once the verification's terminal is durable, so the next verification
proceeds.

## `mod tests` › `fn a_verification_whose_snapshot_destination_cannot_be_made_defers_as_before() {`

The control beside T15: a destination that cannot be made is Git state at once, before `git worktree add`
runs (`workspace_manager`'s destination step, after the add's gate), so `decisions.repairs.not_repairs` applies as it did:
one `merge_verification_unavailable` (Deferred), and the run completes with both candidates
merged. Green before #329 too, where Git's own add failed to make the destination.
