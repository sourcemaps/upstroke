# `src/engine/topology/coordinator.rs`

Extended notes for [`src/engine/topology/coordinator.rs`](../../../../src/engine/topology/coordinator.rs).

The code is the authority for what it does; this file is the whole of its prose but for the
concurrency protocol standards §10 places at its site, which the source keeps in seven comments:
above `PipelineSeams` (what the shared handles are shared for), `SnapshotGate` (the snapshot gate),
`Coordinator` (the coordinator's protocol), `Client` (a pipeline's side of it) and `Timer` (the
wait timer), and in `mod tests` above `TearsAForeignRegistration` and `DelayedRelease` (the two
test workers an owner joins). Each section
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
(`48f2af7a:reviews/2026-09-30-pr11-record.md`, "Phase 3's readings" and "Phase 4's readings"), as the
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
  settlement — and `verify` unwinds when its verification is cancelled, abandoning the integration
  without ending the command (`R1-CONC-3`). The fold keeps a cancelled verification's transaction
  open until its one terminal, which the integration appends after the record of any review passes
  it charged and before admission goes on; `open_in` treats the cancelled verification as closed. A
  halt the fold records is acted on before that (round R3, below).
- **A verification's review spend is charged when a live verification pipeline reports it, accepted
  or discarded, and recorded when no terminal carries it**, before the next selection (phase 4;
  `R1-CONC-4`; the O14(b) change's `merge_verification_charged`).

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

`Wake` is the coordinator's own: its [`Timer`] sends it, at the end of one wait of a registry access
the coordinator makes and with that wait's token, into the coordinator's inbox (`registry_pause`;
the follow-up B record's §9.13, R1, and §9.16 for the one timer). It names no pipeline. A wake that
arrives outside its wait is stale and is dropped.

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
elsewhere there are none) for every pipeline to carry, installs the command's slot limits, starts
the wait timer ([`Timer`]) and then builds the runtime, and drives. It joins every pipeline's handle
and the timer before it returns, whatever the outcome, and then reopens the run if a wait stopped it
(`TopologyRun::stop`; the follow-up B record's §9.16).

### Errors

A refusal before anything is spawned: slot limits that cannot replace the broker's (something is
outstanding in it), a wait timer that could not be started (its thread refused — new at follow-up
B's repair round 6, I2-3: no wait would then be answerable without sleeping on the coordinator), or
a runtime that could not be built. After that, whatever ends the command: a
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
state, the linearization point, a pipeline's transitions, which completion wins, where a
verification's charged passes go, the cleanup after an interrupt, why the unbounded channels are
bounded, and how a registry access the coordinator makes waits only for messages.

## `struct Coordinator<'s>` › `deferred: VecDeque<(Origin, ToCoordinator)>,`

Completions a registry access's wait received and deferred, applied first by [`Self::next_message`]
once the transition returns.

## `struct Coordinator<'s>` › `wakes: u64,`

The last wait's token; each wait takes the next, so a stale wake never ends a later wait.

## `struct Coordinator<'s>` › `timer: Timer,`

The one thread every wait is timed by, started before the coordinator acts ([`Timer`]).

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

Whether `verify` is on the stack and its verification is no longer open — a decline, or a lineage
member's failed settlement, failed the lineage and cancelled the verification, whose transaction the
fold holds until its terminal (`closure::cancelled_by_lineage`), and `reconcile` has stopped the
verification's pipeline. Admission ends its pass while it is, so the next thing `verify` does is wait
for that pipeline to end and unwind. Admitting other work under a `verify` that is still returning
once selected a candidate already queued for integration and refused it inside `verify`, which ended
the command (round R2, `R2-DECLINE-QUEUED`).

## `impl Coordinator<'_>` › `fn integrate(&mut self, candidate: CandidateRef) -> Result<bool, UpstrokeError> {`

An integration, on the coordinator's thread, through the frozen `integrate()` over
[`DrivenJournal`]. Stale is predicted before anything is taken — the candidate's recorded base is
not the log's authorized head, which is `decide`'s own test once the ref is not foreign (a foreign
head refuses before any staging or reclaim) — and while an attempt snapshot is live a stale
integration takes no reservation: the gate closes, admission stops, messages are applied, and when
no attempt snapshot is live selection runs afresh (an answer ingested meanwhile can put another
candidate at the head of the queue). A fast integration never reclaims and does not wait. `false`
ends the admission pass: nothing was started, or the command is already ending — an error the
interrupt already accounts for is not a second error. A verification abandoned because a lineage
failure cancelled it (`abandoned`) ends the integration without ending the command: `integrate()`
appends nothing after `verify`'s error, so the coordinator settles it here, in two appends. First it
records the passes the verification charged, if any (`merge_verification_charged`), which the fold
admits because it holds the cancelled verification's transaction open until its terminal. Then, before
the gate reopens and admission goes on, `settle_cancelled_verification` appends its one terminal,
`merge_verification_interrupted`, and reclaims its pin expected-old, its staging worktree and intent,
and its snapshots, as `close_run` settles each in-flight item. `verify` returns `Abandoned` only with
no interrupt recorded and once the pipeline is gone, so the Runner has established the end of each of
its processes. An error of either append ends the admission pass under the append-error protocol, and
the record's own error is recorded as the interrupt (`supersede`), so no terminal follows it; the
resume settles the verification from the surviving prefix. An interrupt ends the integration with no
terminal appended, and the halt's closure or the next resume settles the verification instead.

While the run is stopped, the recording point takes nothing. The stop means a registry access's wait
inside another transition met the interrupt that ended this integration, as when its verification's
result had arrived and the gate was draining attempt snapshots. The passes it kept stay in `charged`
for `finish`, which records them past the stop once every transition has returned, so this
integration appends nothing after the stop (`O14B2-1`).

The settlement of a verification a lineage failure cancelled takes nothing then either, and the
interrupt guard is what keeps it off a stopped run: `registry_pause` stops a run only once an
interrupt is recorded, and `verify` returns `Abandoned` only with none recorded, with no registry
access between that return and the settlement. So a stop in the window after the lineage failure
ends `verify` as interrupted, `finish` records the passes past the stop, the command ends with no
terminal appended, and the next resume's recovery step (f) appends the cancelled verification's one
terminal after the record.

## `impl Coordinator<'_>` › `fn idle(&mut self) -> Result<Progress, UpstrokeError> {`

The idle-only arms, by the functions `step` runs: the backoff, the hard block, the closure. Any
other arm here is a refusal, because the admission pass takes it first. The hard block and the
closure run with the coordinator as their operator (`run::hard_block`, `run::close_run`), so their
registry waits, finalization's included, answer its messages (the follow-up B record's §9.16, I2-2).

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
snapshot the reclaim that follows removes is the integration's own. When a lineage failure
cancels the verification — a decline, or a lineage member's failed settlement, failed its lineage —
the admission pass has stopped it; `verify` waits for its pipeline to end and returns an error that
abandons the integration (`R1-CONC-3`), so it never waits on an inbox its verification will not
write to.

### Errors

A refusal when asked inside another verification (one transaction is open at a time); the
verification job's own refusals; and, when an interrupt ends the command first, a refusal naming
it — the pipeline was cancelled, `integrate()` appends no terminal for it, and the coordinator
records the passes it charged, if any (`merge_verification_charged`), before the command ends. The sequence is
recorded as cancelled, whether or not its result had already arrived, so a halt's closure settles
the transaction with `merge_verification_interrupted` (R-AF) — a halt recorded after the result
arrived and before this returned included (round R2: the admission pass's halt check reads the fold's
in-flight work, not the live table). And, when a lineage failure has cancelled the verification, a
refusal naming the cancellation — the fold holds its transaction only until its one terminal,
appended once its pipeline has ended — which `integrate` reads through `abandoned`, answers by
appending that terminal, and does not treat as the command's end; admission selects nothing until
then (`abandoning`).

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

## `impl Coordinator<'_>` › `fn keep_discarded(`

A live verification pipeline's completion that is discarded — its pipeline cancelled (a halt, a
shutdown, an error elsewhere, an unresolved end), its transaction released, or the fold poisoned —
still carries the passes its pipeline's account charged as each returned. They are charged to the
run's spend here, which master never did, and kept for recording. Only a completion from a pipeline
of this coordinator, live under the identity it names, is kept; stale, foreign and injected ones are
not. An accepted completion is kept in `verified_arrived` beside its live charge, before `verified`
consumes the vector.

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

## `impl Coordinator<'_>` › `fn supersede(&mut self, append: UpstrokeError, ended: Option<&UpstrokeError>) {`

The record's own append error, made the command's. `fail` keeps the first interrupt and only warns of
a later error, which is right for a second error while the command is already ending and wrong for
this one: T-APPEND makes a returned append error the command's error. So the stored interrupt is
replaced and kept as a warning (`superseded`): the stored error, a halt's or a shutdown's description,
or else the frozen `integrate()`'s own error. No closure follows a halt it replaced, and none could
append on the poisoned fold. It is called only at the two recording points. A stopped run does not
refuse the record: `integrate`'s recording point leaves it to `finish`, which offers it with the stop
lifted (`TopologyRun::record_charged_past_the_stop`), so what reaches `supersede` there is the record's
own append error alone (`O14B2-1`).

## `impl Coordinator<'_>` › `fn finish(&mut self) -> Result<Progress, UpstrokeError> {`

End the command: cancel, apply messages until no pipeline is live, then act on the interrupt.
After a halt the closure runs, with the coordinator as its operator (`run::close_run`; I2-2), with
the identities this coordinator cancelled — unless a cancelled
pipeline ended with its process unresolved, in which case nothing is appended and the command ends
resumably naming it. Before any of that, the passes a verification charged and no terminal carries
are recorded (`merge_verification_charged`): a completion discarded in the receive loop, or one kept
earlier and not yet recorded. On a stopped run that record is offered with the stop lifted for the
offer alone (`TopologyRun::record_charged_past_the_stop`): every transition has returned by then, so it
is appended for none of them, and the stop holds again for everything after it (`O14B2-1`). A shutdown cancels any provisional reservation still held (none is
expected) and ends resumably, appending nothing but that record. An error is returned as it is: after
an append error it is the protocol's report, and no closure, report or cleanup follows. The record's
own append error is the exception that ends the command in place of what was stored: `supersede`
makes it the interrupt and keeps what it replaced as a warning.

## `struct Coordinator<'s>` › `cancelled_work: closure::Cancelled,`

The in-flight identities this coordinator cancelled or abandoned, which a halt's closure settles.

## `struct Coordinator<'s>` › `charged: Option<Charged>,`

The passes of the one open verification, kept from its completion until they are recorded or a
terminal carries them. Taken, never copied, at both recording points: after the frozen
`integrate()` returns an error, and after `finish`'s receive loop. On a stopped run `integrate` leaves
it, and `finish` takes it. One verification is open at a
time and each pipeline sends one completion, so at most one is kept.

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
(`run::begin_dispatch`, `run::begin_retry`, `run::settle_judged`, `run::hard_block`,
`run::close_run`, and through `DrivenJournal` the frozen `integrate()`).

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

**A stopped run's effects are refused** (the follow-up B record's §9.16, I2-1). Once the run is
stopped (`TopologyRun::stop`, below), `phase` answers `Error` at `Before`, at `After` and at an
error-return point, with the cause "nothing further is done in this process: <why>", so every
effect a transition makes through these hooks — a worktree, an intent, a ref, the integration ref's
compare-and-swap — is refused before its primitive runs; at a point whose only other answer is
`Kill` it proceeds, since an `Error` there would invent an error-return contract the point does not
declare. With the run's emitter refusing every append a transition makes, a transition that carried
on past a stopped wait publishes and appends nothing; the one append past the stop is `finish`'s
record of the passes a verification charged (`fn finish`).

**`registry_pause`** is one wait of an access a transition makes. It asks the [`Timer`] for a wake
after the wait's length — no thread per wait, so a wait needs no thread it might not get (I2-3) —
and answers messages until that wake ([`Self::answer_until_woken`]); a timer that cannot be asked
is a typed refusal recorded as the command's error, never a sleep. A wait that ends with an
interrupt recorded — a shutdown it answered, an error a message it answered raised, or that refusal
— stops the run and returns an error ("a registry access's wait inside a transition met <the
interrupt>…"), so the access stops before another attempt (`tolerant_registry_access`) and the
transition propagates it (I2-1). A wait that begins with an interrupt already recorded does not
wait. Halt and budget judgements stay deferred until the transition returns, as completions are.

## `struct Timer {`

The coordinator's wait timer: one thread for the coordinator's life, which the source's comment
beside the struct gives the protocol of (§10). The coordinator starts it before it builds its
runtime, so a thread it cannot have refuses the command before anything is spawned or appended, and
stops and joins it when it ends.

## `impl Timer` › `fn start(outbox: mpsc::UnboundedSender<ToCoordinator>) -> Result<Self, UpstrokeError> {`

The timer, sleeping with `std::thread::sleep`.

## `impl Timer` › `fn starting(`

The timer with the sleep given: production's is `std::thread::sleep`, and a test's can unwind, to
show that a sleep that unwinds is still answered and that the thread goes on serving. Each request's
sleep runs under `catch_unwind`, and the wake is sent after it either way, so a wake the coordinator
waits for cannot be lost while the thread lives; the thread ends only when the coordinator drops its
sender (the follow-up B record's §9.16, I2-5).

## `impl Timer` › `fn wake(&self, token: u64, pause: std::time::Duration) -> Result<(), UpstrokeError> {`

Ask for `Wake { token }` after `pause`. A timer that has stopped cannot be asked: that is a typed
refusal, and the wait it would have timed is not slept.

## `impl Timer` › `fn stop(&mut self) -> Option<String> {`

Drop the sender, join the thread, and say what to warn of: sleeps that unwound, or a thread that
did. Idempotent; the drop calls it.

## `struct Client {`

The source keeps a pipeline's side of the protocol beside the struct (§10). `gated` is true for an
attempt's pipeline and false for a verification's.

## `impl Client` › `fn ask(&self, message: impl FnOnce(Reply) -> ToCoordinator) -> Result<(), UpstrokeError> {`

Send a request and wait for its reply on the pipeline's thread. A coordinator that has stopped
listening is a refusal, never a hang.

## `impl Registrar for Client {`

A pipeline's registrar (R-U): `admit` asks and waits for the grant, `ended` notifies, and the two
snapshot calls ask and notify when the pipeline is gated.
