# `src/engine/topology/scaffold.rs`

Extended notes for [`src/engine/topology/scaffold.rs`](../../../../src/engine/topology/scaffold.rs).

The code is the authority for what it does; this file is the whole of its prose, moved out of
the source verbatim. Each section is headed by the line of code the comment sat above, spelled
as it is in the source, so the heading is the grep string that finds the code.

## Module

The schema-4 run a dispatch or attempt test drives.

Shared by [`super::dispatch`] and [`super::attempt`] because the two halves
of one lifecycle are tested against one run: an attempt test needs a
dispatched generation, and a dispatch test needs the attempt that never
started. A second run fixture beside this one would be two hand-maintained
copies of a `run_started` record — the class this crate has recorded three
times.

### Why the effects come from `workspace_manager::fixture`

`src/engine/topology/**` is a topology module. `clippy.toml` denies
`std::fs::write`, `std::fs::create_dir_all` and `std::process::Command`
there, **including in `#[cfg(test)]` code** — measured. Everything this
module needs that no funnel owns (`git init`, bytes in a worktree, a child
to kill) therefore comes from
[`crate::workspace_manager::fixture`], which is inside the reviewed funnel
module. Nothing here carries an `allow`.

## `pub(super) const ALPHA: TaskKey = TaskKey(0);`

The two plan tasks every fixture run carries.

## `pub(super) const BETA: TaskKey = TaskKey(1);`

The second, so an assertion about "this task" can be crossed against
another whose generations move independently.

## `pub(super) const AGENT: &str = "claude-code";`

The agents this fixture's pre-flight probed.

## `pub(super) const REVIEW_AGENT: &str = "copilot";`

A second, so a slot pair taken for the worker is distinguishable from one
taken for a reviewer.

## `const NORMALIZED_DIGEST: &str =`

The digest the fold authenticates the normalized plan against. A literal
because this fixture never writes a `plan.normalized.json`; the fold
compares it to `run_started`'s own field and to nothing on disk.

## `fn run_started(fixture: &Fixture) -> RunStarted4 {`

A `run_started` for a real repository: the execution root, the private root
and the base commit are the fixture's own, so an event this run appends is
checkable against the directory the funnels actually touched.

## `pub(super) struct FoldedEmitter {`

---------------------------------------------------------------------------
The emitter
---------------------------------------------------------------------------

## `pub(super) struct FoldedEmitter {`

`coordinator_integration.emit`, minus the append-error protocol.

"build event → serialize → round-trip → `plan_transition` → append the exact
bytes through the Event funnel", which is what an emit *is* when it
succeeds. The protocol for when it does not is `emit.rs`'s (O17) and is
deliberately absent: a second implementation of it living in a test fixture
would be a second thing to keep in step with the one that ships.

Owns its own [`crate::events::log::HarnessEventHooks`] over the **shared**
harness rather than borrowing the effect bundle's, so an ordering assertion
can read the append and the worktree add off one observation list while the
two values are borrowed independently.

## `impl FoldedEmitter` › `pub(super) fn fold(&self) -> &TopologyFold {`

The fold, for the state an event is supposed to have produced.

## `impl FoldedEmitter` › `pub(super) fn durable_events(&self) -> Vec<TopologyEvent> {`

Every event in the log on disk, replayed from its bytes.

## `impl FoldedEmitter` › `pub(super) fn durable_kinds(&self) -> Vec<&'static str> {`

The kinds in the log on disk, in order.

## `impl FoldedEmitter` › `pub(super) fn task(&self, key: TaskKey) -> &TaskFold {`

One task's fold state.

## `impl FoldedEmitter` › `pub(super) fn generation_class(`

The class of the generation `generation` of `key`.

## `impl EventEmitter for FoldedEmitter` › `fn standing(&self, invocation: &InvocationId) -> super::select::Standing {`

The standing the production emitter reports, read from this emitter's own
fold, so an attempt driven through the scaffold registers its slotted
processes under the same precondition as the run's.

## `impl EventEmitter for FoldedEmitter` › `fn emit(`

**`_hooks` is ignored, and that is the divergence rather than an
oversight.** This emitter's own `EventHooks` is a `TimelineEvents`,
which records each `(site, phase)` into the ordering timeline as well as
into the harness. The shared bundle's `events` family is a bare
`HarnessEventHooks` and does not. Using the parameter here would
silently drop every append out of the timeline, and the ordering
assertions that read it would go green having stopped observing the
thing they order.

The repair is to give the shared bundle the timeline wrapper, not to
take it away from here — but that is a change to test infrastructure
every topology test depends on, which is the shape PR5's round 7 was
reverted for. Recorded instead.

## `pub(super) struct Timeline(Arc<Mutex<Vec<(EffectSiteId, HookPhase)>>>);`

---------------------------------------------------------------------------
The hook bundle, with the two phases the shared harness cannot arm
---------------------------------------------------------------------------

## `pub(super) struct Timeline(Arc<Mutex<Vec<(EffectSiteId, HookPhase)>>>);`

Every `(site, phase)` any funnel reached, in order, with repeats.

The [`HookHarness`] cannot answer an ordering question on its own, and this
is not a defect in it: `coverage()` is a **set** in first-observation order,
because what it exists to prove is that every site executed at least once.
An ordering clause is about *occurrences* — O24 is "verification, then the
retry's append", and by the time a retry runs, both sites have already been
observed once by the dispatch that opened the generation, so a comparison of
first observations is a comparison of the wrong pair and passes or fails for
the wrong reason. Measured: it failed here, on a `retry` whose order was
right.

So the timeline is a second, ordered record kept beside the harness, fed by
the same calls. The harness still receives everything — nothing here
replaces an observation, it only adds one — so the coverage evidence is
unaffected.

## `impl Timeline` › `pub(super) fn positions(&self, site: EffectSiteId, phase: HookPhase) -> Vec<usize> {`

Every position at which `(site, phase)` was reached.

## `impl Timeline` › `pub(super) fn mark(&self) -> usize {`

How much has happened so far — a fence a later assertion counts from.

## `struct TimelineEvents {`

[`EventHooks`] that record into the shared harness and onto the timeline.

`EventHooks::phase` returns nothing — for that family the two phases are
reachability rather than injection points — so this adds no arming, only the
ordered record an append's position in a clause needs.

## `pub(super) struct ArmedEffects {`

[`EffectHooks`] that record into the shared [`HookHarness`] **and** can be
armed at a `Before` or `After` phase.

[`HookHarness::arm`] takes a [`SubEffectPoint`], and `HookHarness::hook`
answers `Proceed` to both phases unconditionally — deliberately: a phase is
reachability, not an injection coordinate. But `T-DISPATCH` and `T-ATTEMPT`
table prefixes that are exactly "between these two effects", and the only
place to stand between two funnels is a phase of one of them.

So the arming is local and the **recording is not**: every call reaches
`HarnessEffects` first, so the observation lands in the one harness
`check_bijection` reads, and only the answer is this type's.

An armed answer is handed back through `Exported::carried`, so a `Kill` writes
the observation export before the funnel aborts. `HarnessEffects` exports
before a kill only when the harness itself answers one, which it never does
at a phase, so until Gate 5's strict re-audit a kill child dying at an armed
phase lost the record of the coordinate it died at, and no registry claim
could name the child for that coordinate.

## `impl ArmedEffects` › `pub(super) fn arm(&mut self, site: EffectSiteId, phase: HookPhase, injection: Injection) {`

Answer `injection` the next time `site` reaches `phase`.

## `fn phase(&mut self, site: EffectSiteId, phase: HookPhase) -> Injection {` › `self.timeline.push(site, phase);`

Recorded first and unconditionally: an armed site is still a site the
suite executed, and a bundle that skipped the harness when it had an
answer of its own would drop exactly the observations the fault tests
produce.

## `impl EffectHooks for ArmedEffects` › `fn refusal_cause(&self) -> Option<String> {`

Forwarded, so a poison the inner observer found is reported as poison
and not as a fault this bundle armed; the method is not defaulted for
exactly this reason.

## `pub(super) struct Hooks {`

The five families, with [`ArmedEffects`] in the git seat.

## `impl Hooks` › `pub(super) fn arm(&mut self, site: EffectSiteId, phase: HookPhase, injection: Injection) {`

Arm a phase of a git funnel site.

## `pub(super) struct Ran {`

---------------------------------------------------------------------------
The runner double
---------------------------------------------------------------------------

## `pub(super) struct Ran {`

One request the fake runner was given.

## `pub(super) struct Ran` › `pub(super) invocation: InvocationId,`

Which identity it carried.

## `pub(super) struct Ran` › `pub(super) role: ExecutionRole,`

Which seat it occupied.

## `pub(super) struct Ran` › `pub(super) workspace: PathBuf,`

Where it ran.

## `pub(super) struct Ran` › `pub(super) agent: Option<AgentId>,`

The agent it was bound to, if any.

## `pub(super) struct Ran` › `pub(super) command: CommandSpec,`

Its program and arguments.

## `pub(super) struct Ran` › `pub(super) request: RunnerRequest,`

The whole request, as given — `tests_acceptance.determinism` asks for "recorded
RunnerRequests" beside their `InvocationId`s; the fields above are the ones
older tests read by name.

## `pub(super) struct Ran` › `pub(super) policy: RunnerPolicy,`

The `RunnerPolicy` this runner declared the invocation executed under:
`host_policy()` unless [`RecordingRunner::declaring`] named another. ST-20 asks
that "the FakeRunner/fake container runtime record that every probe and
invocation of the resumed epoch executed under the recorded boundary and image
id"; this is the runner's half of that record.

## `pub(super) struct Ran` › `pub(super) image_id: Option<String>,`

The immutable image id of the declared policy, when it names an image — `None`
for a host boundary.

## `pub(super) struct Ran` › `pub(super) durable_at_spawn: Vec<String>,`

The event kinds the log **on disk** held at the instant this process was
requested.

The only oracle O23 has. "`attempt_started` before spawn" is a claim
about two things that happen at two moments, and every other record here
is read *after* both — so a `start` that spawned first and appended
afterwards leaves an identical `Ran`, an identical durable log and an
identical fold. Measured: with the append moved after the spawn, the
whole of this test stayed green until this field existed.

## `pub(super) const GATE_DIAGNOSTIC: &str = "scaffold gate rejected the diff";`

A [`Runner`] that runs nothing and records everything.

The engine is the conductor: it never implements an agentic loop and never
calls a model. What a test of *ordering* needs from the runner is therefore
not an execution but a record — which identity, which seat, which workspace
— and the workspace is the load-bearing one here, because
`decisions.workspace_candidates.snapshots` says gates and reviewers execute
only in exact snapshots and "worker worktrees and the staging worktree are
never used for verification processes".
What a refused scaffold process prints, so a test can follow it into the
feedback a retry is given.

## `pub(super) enum ProbeFailure {`

How a probe the test told to fail fails: a process that ran and exited
non-zero with the given stderr (the CLI is there and broken), or a spawn
that never started (the CLI is not there). `tests_acceptance.determinism`:
"the ability … to fail a shell or agent probe on demand".

## `pub(super) enum Ending {`

How an invocation this runner was handed ended: completed (an output),
failed (a Runner error the test delivered, or a probe told to never start),
cancelled (its call's cancellation fired while it was held), cancelled before
start (its call was cancelled before the runner started it: no process, nothing
in [`RecordingRunner::ran`]), cancelled unresolved (cancelled while held, and
answered with a Runner error whose process fate is unresolved, as a container
runner answers when it cannot establish that the container is gone —
[`RecordingRunner::unresolved_when_cancelled`]), abandoned (its future was
dropped while held).
Recorded once per invocation, in the order the endings happened; for a completion
the moment is the test's delivery, so the order is the test's and not the order
the driving threads happened to resume. Cancellations that one interrupt fires
together are recorded in the order their threads resume, which is unordered.

A call cancelled before it started was once answered and left no trace. An oracle
reading the endings was then blind to exactly the cancellations that land between
a grant and the Runner, which is how phase 3's shutdown test came to depend on
thread timing (the working record's §13, round C1; R-AE).

## `struct Held {`

One invocation waiting for the test: the waker of the future that last polled
it, and the result once the test delivers one.

## `struct Door {`

A call waiting at the runner's door, not yet started: whether it has been
admitted, and the waker of the future that last polled it.

## `struct Control {`

The runner's control state behind one lock: the declared policy, whether
invocations are held, whether calls enter late and which are barred, which
answer unresolved when cancelled and which panic when released, the calls
waiting at the door, the probe failures still owed, the held invocations, the
endings, and how many completions were refused. `changed` (a `Condvar` on the
same lock) is notified whenever an invocation reaches or leaves the door, starts
waiting or ends, so a test can wait for "three are in flight" instead of sleeping.

## `impl Control` › `fn inside(&self, invocation: &InvocationId) -> bool {`

Whether the runner has the call where a test can rely on it staying: held with no
result delivered, or barred at the door. A call admitted but not yet started, or
whose result has been delivered, is on its way somewhere and is not inside.

## `pub(super) struct RecordingRunner` › `codes: Mutex<Vec<i32>>,`

Exit codes to hand back, in order. Exhausted entries answer 0.

## `pub(super) struct RecordingRunner` › `log: Mutex<Option<PathBuf>>,`

The run's event log, read at the instant of each request.

## `impl RecordingRunner` › `pub(super) fn new() -> Self {`

A runner every process succeeds under.

## `impl RecordingRunner` › `pub(super) fn set_codes(&self, codes: Vec<i32>) {`

Replace the queued exit codes on a runner already in a `Run`.

`failing_with` builds one; a test that needs the fixture's whole run and
only wants different codes cannot rebuild it, because the `Run` owns the
runner and its harness.

## `impl RecordingRunner` › `pub(super) fn failing_with(codes: Vec<i32>) -> Self {`

Hand back these exit codes, in order, then zeroes.

## `impl RecordingRunner` › `pub(super) fn watching(&self, log: &Path) {`

Read `log` at the instant of every request, so an ordering clause about
"before any spawn" has something to be true *at*.

## `impl RecordingRunner` › `fn durable_now(&self) -> Vec<String> {`

The kinds the log on disk holds right now, read through its last newline: at
width > 1 a pipeline's process can start while the coordinator is mid-append,
and a torn last line is the append in flight, not a log that fails to parse.

## `impl RecordingRunner` › `pub(super) fn ran(&self) -> Vec<Ran> {`

Every process it started, in order. A call cancelled before it started, or still
waiting at the door, is not here; its ending, or [`RecordingRunner::inside`], says
where it is.

## `impl RecordingRunner` › `pub(super) fn declaring(self, policy: RunnerPolicy) -> Self {`

Declare the policy every invocation of this runner records as the one it ran
under (a container policy with an image id, say, for a test of a resumed epoch).

## `impl RecordingRunner` › `pub(super) fn hold(&self) {`

From now on every invocation waits, after it is recorded, until the test
delivers its result with [`RecordingRunner::complete`] or its call is
cancelled — `tests_acceptance.determinism`'s "explicit start/complete control".
Without it, the runner answers each request at once from the queued codes, as
before PR11.

## `impl RecordingRunner` › `pub(super) fn stop_holding(&self) {`

From now on invocations are answered at once again; those already held stay
held until completed. A test that holds one invocation mid-run and then
drives another through the same runner uses it, so that the second — if a
regression let it reach the runner — returns at once and fails its assertion
instead of hanging the test.

## `impl RecordingRunner` › `pub(super) fn enter_late(&self) {`

From now on every call waits at the door, before its cancellation is checked and
before anything is started or recorded, until a test admits it
([`RecordingRunner::admit`]) or its call is cancelled, so a call starts only when a
test lets it in. The coordinator's test scheduler turns it on, so a coordinator
that acts before a granted pipeline has reached the Runner meets that pipeline
still outside, every time.

## `impl RecordingRunner` › `pub(super) fn bar(&self, invocation: InvocationId) {`

This call waits at the door until its call is cancelled, admitted or not: a
granted invocation whose process never starts, the position a cancellation finds
when it lands between a grant and the Runner. It then ends
[`Ending::CancelledBeforeStart`].

## `impl RecordingRunner` › `pub(super) fn admit(&self, invocation: &InvocationId, within: Duration) -> bool {`

A grant's other half: under late entry, wait until the call reaches the door,
let it in, and return once it is held — or at once when it is barred and at the
door; without late entry, wait until the call is held — or `false` when `within`
passes. A test scheduler calls it for every invocation the coordinator
grants, so each process starts, and reads the log it records, while the
coordinator waits. It needs a holding runner: a call answered at once is never
inside.

## `impl RecordingRunner` › `pub(super) fn panic_when_released(&self, invocation: InvocationId) {`

When this held invocation is released, its future panics on the pipeline's
thread as it hands back the delivered result, so the Runner call unwinds and its
caller never reports the end: the position in which a coordinator must not
release what the pipeline held (the working record's §13, round R1).

## `impl RecordingRunner` › `pub(super) fn unresolved_when_cancelled(&self, invocation: InvocationId) {`

When this invocation's call is cancelled while held, answer it with a Runner
error whose process fate is unresolved rather than a cancellation, and record it
[`Ending::CancelledUnresolved`]: the position a coordinator's halt must not close
over (the working record's R-AF).

## `impl RecordingRunner` › `pub(super) fn inside(&self, invocation: &InvocationId) -> bool {`

[`Control::inside`], for a test that checks without waiting.

## `impl RecordingRunner` › `pub(super) fn fail_probe(&self, target: ProbeTarget, failure: ProbeFailure) {`

The next probe of `target` fails as `failure` says, once.

## `impl RecordingRunner` › `pub(super) fn await_waiting(&self, count: usize, within: Duration) -> Vec<InvocationId> {`

Block until at least `count` invocations are waiting, or `within` passes, and
return the ones waiting — the handshake a test takes before it delivers
completions, so it never delivers to an invocation that has not started.

## `impl RecordingRunner` › `pub(super) fn complete(`

Deliver one held invocation's result. The Runner side of a completion is
exactly-once: a second delivery for the same invocation, or one for an
invocation this runner never held (or already ended), is refused and counted
(`refused_completions`). The stale, duplicate and out-of-order *completions*
PR11's coordinator must discard (ST-01, ST-02, ST-03) are completions a
pipeline delivers to the coordinator, and belong to phase 3; what this gives
phase 3 is the control underneath them — invocations that end in the order the
test chooses, with the result it chooses, each exactly once.

## `impl RecordingRunner` › `fn start(&self, request: &RunnerRequest) -> Started {`

The first poll of an invocation: record it (with the declared policy and image
id), answer a probe the test told to fail, hold it when holding, or answer at
once — from the responder when one is set ([`RecordingRunner::answering`]),
otherwise from the queued codes.

## `fn start(&self, request: &RunnerRequest) -> Started` › `let durable_at_spawn = self.durable_now();`

Read before the request is recorded, so what it captures is the log
as it stood when the process was asked for.

## `fn start(&self, request: &RunnerRequest) -> Started` › `stdout: if code == 0 {`

A refused process says something, the way a real one does: §11.1
makes the tail the feedback a retry is given, and a fixture whose
processes print nothing cannot tell a carried tail from a dropped
one.

## `impl RecordingRunner` › `fn settle_held(`

A later poll of a held invocation: hand back a delivered result (or panic, for
an invocation `panic_when_released` names), or end the invocation cancelled if
its call's cancellation fired (registering the waker first, so a cancel racing
the poll is never lost), or store the waker and wait.

## `impl RecordingRunner` › `fn abandon(&self, invocation: &InvocationId) {`

The future of a held invocation was dropped before it ended: release the hold
and record it abandoned — the double's analogue of a Runner terminating an
invocation whose caller went away. An invocation whose result was already
delivered has its ending already and records nothing more.

## `impl RecordingRunner` › `fn at_the_door(`

Whether a call not yet started must wait: only when calls enter late or it is
barred, and then until it is admitted (a barred call never is) or its call is
cancelled. The waker is registered with the cancellation first, so a cancel
racing the poll is never lost, and with the door, so an admission wakes it.

## `impl RecordingRunner` › `fn leave_door(&self, invocation: &InvocationId) {`

A future dropped at the door takes its call away with it; nothing started, so
nothing is recorded.

## `impl RecordingRunner` › `fn cancelled_before_start(&self, invocation: &InvocationId) {`

Record a call whose cancellation fired before the runner started it.

## `struct Invocation<'a> {`

The future `run` returns: its first poll waits at the door when it must, then
answers a call already cancelled as cancelled before start (fate `NeverStarted`,
recorded), and otherwise starts the invocation; later polls settle it, and its
`Drop` abandons an invocation still held or takes a waiting call from the door.

## `impl Runner for RecordingRunner` › `fn run<'a>(&'a self, request: &'a RunnerRequest, call: RunnerCall<'a>) -> RunFuture<'a> {`

Every request becomes an [`Invocation`] future carrying the call's
cancellation. Unheld, it resolves in its first poll, like the host and
container runners' futures.

## `pub(super) struct AnsweringAdapter {`

---------------------------------------------------------------------------
The agent boundary, doubled
---------------------------------------------------------------------------

## `pub(super) struct AnsweringAdapter {`

An agent CLI that answers, without an agent CLI.

**A double, not a re-implementation.** It implements the real
[`AgentAdapter`] trait, builds a real [`CommandSpec`], and every invocation
it produces is spawned through [`RecordingRunner`] — so what the tests
observe is the engine's own request, at the engine's own boundary. Nothing
here re-implements what an adapter *does*; it stands in for what an adapter
*talks to*.

It exists because the scaffold used to name real adapter ids —
`claude-code` and `copilot` — which `BuiltinAdapters` resolves, sending
`run_review` off to locate an actual CLI that is not there. A fixture that
points at a real boundary and hopes is the same defect as a fixture that
invents a shape production never builds, arriving from the other side.

`review.rs`'s three private fakes were considered and are the wrong shape:
`NeverInvokedAdapter` panics on every method by design, and the other two
model an outage and a deadline. All three are **negative-case** doubles, and
teaching one to answer would change what it means for the tests that own it.

## `pub(super) struct AnsweringAdapter` › `status: crate::ir::OutcomeStatus,`

What this agent reports about its own run.

A field rather than a constant because an **outage** is a distinct path
through the ladder and needs a fixture that reaches it: `RateLimited` is
what `AttemptFailure::is_outage` recognises, and it is the difference
between an attempt that spends one of its rung's allowances and one that
defers spending none.

## `impl AnsweringAdapter` › `pub(super) const fn erroring(id: &'static str) -> Self {`

A reviewer that passes.
An agent whose CLI reports its own error.

`FailureKind::AgentError` is neither an outage nor a question, so
`next_step` retries on the same rung while the allowance lasts — and
`resume: true` when the agent can resume and returned a session, which
is what makes the generation `Retained` rather than closed.

## `impl AnsweringAdapter` › `pub(super) const fn asking(id: &'static str) -> Self {`

An agent that stops and asks rather than working.

`evaluate_outcome` reads `UPSTROKE-QUESTION:` out of the outcome's
detail **before** the evidence rules, because "an agent that stopped to
ask has not failed at anything". That is `FailureKind::NeedsHuman`,
which `next_step` sends straight to a park.

## `impl AnsweringAdapter` › `pub(super) const fn rate_limited(id: &'static str) -> Self {`

An agent whose CLI reports it is rate-limited: `evaluate_outcome` maps
that to `FailureKind::RateLimited`, which `is_outage` recognises and
`next_step` defers rather than blames on the implementer.

## `fn build(&self, run: &crate::agent::TaskRun) -> Result<CommandSpec, UpstrokeError> {` › `let spec = CommandSpec::new(self.id)`

A real spec, carrying the prompt, so a test that reads the recorded
command sees what was actually asked for.

**And the session, when there is one to resume.** Every real adapter
puts it in argv; one that dropped it here would make a retry that
lost its session indistinguishable from one that kept it, which is a
fixture blind spot rather than a simplification — measured, by a
mutation that survived until this line existed.

## `impl crate::agent::AgentAdapter for AnsweringAdapter` › `Ok(crate::ir::Outcome {`

`detail` is where `run_review` reads the verdict from, and `cost_usd`
is what `ReviewRecord` requires — both come from the adapter because
both are things only the agent's own CLI knows.

## `pub(super) struct ScaffoldAdapters {`

The scaffold's adapters: the two the fixture's plans name, and nothing else.

Deliberately not `BuiltinAdapters`. An unknown agent must be a refusal a
test can see, not a silent fall-through to a real CLI.

## `impl ScaffoldAdapters` › `pub(super) const fn erroring() -> Self {`

The same two agents, with the implementer reporting its own error.

## `impl ScaffoldAdapters` › `pub(super) const fn asking() -> Self {`

The same two agents, with the implementer stopping to ask.

## `impl ScaffoldAdapters` › `pub(super) const fn rate_limiting() -> Self {`

The same two agents, with the implementer reporting a rate limit.

## `fn scaffold_run_paths(fixture: &Fixture) -> crate::rundir::RunPaths {`

The run directories a scaffold `Run` writes transcripts, settings and reviews to, with the
private half inside the fixture's own tree.

They were `RunPaths::new(<repo>, "01SCAFFOLD00000000000000AA")`, whose private half is the
**default** private root, `~/.upstroke`: one fixed directory that every scaffold run in every
process on the machine wrote into, over whatever the last one left, and that nothing removed.
Under the fixture's tree it is this run's alone and goes with the tree.

## `pub(super) struct Run {`

---------------------------------------------------------------------------
The run
---------------------------------------------------------------------------

## `pub(super) struct Run {`

A real repository, a real event log, a fold over it, and the five hook
families on one harness.

## `pub(super) struct Run` › `pub(super) paths: crate::rundir::RunPaths,`

The run's directories, for the seams that write under them.

## `pub(super) struct Run` › `pub(super) harness: Arc<Mutex<HookHarness>>,`

The one harness every family records into.

## `pub(super) struct Run` › `pub(super) hooks: Hooks,`

The five families.

## `pub(super) struct Run` › `pub(super) timeline: Timeline,`

Every `(site, phase)` in order, with repeats.

## `pub(super) struct Run` › `pub(super) emitter: FoldedEmitter,`

The log, the fold, and the emit sequence over them.

## `pub(super) struct Run` › `pub(super) runner: RecordingRunner,`

What a spawn would have been.

## `pub(super) struct Run` › `pub(super) invocations: crate::engine::topology::identity::InvocationLedger,`

The R4 ledger this fixture discharges obligation (3) against. In
production the driver owns it; here the fixture is the caller.

## `pub(super) struct Run` › `pub(super) fixture: Fixture,`

The repository and its manager.

Last, because fields drop in declaration order and this one's guard reclaims the tree the others
live in. `emitter`'s `EventLog` holds `events.jsonl` inside that tree open until it drops;
whether Windows removes a file under an open handle depends on how the handle was opened, and
closing it first leaves nothing to depend on.

## `impl Run` › `pub(super) fn started(tag: &str) -> Self {`

A started schema-4 run over a fresh repository.

## `pub(super) fn started(tag: &str) -> Self` › `let paths =`

`RunPaths`'s own doc: "Callers do this once at run start;
every accessor below assumes it has happened." The scaffold
was handing out paths without it, so a review's transcript
write failed into `unavailable_after_error` and the pass was
recorded as an OUTAGE — which spends no attempt. A fixture
that skips a documented precondition does not fail loudly;
it produces a plausible wrong answer.

## `impl Run` › `pub(super) fn manager(&self) -> &WorkspaceManager {`

The manager.

## `impl Run` › `pub(super) fn base(&self) -> CommitSha {`

The base every dispatch of this fixture is made at.

## `impl Run` › `pub(super) fn predicted(&self, key: TaskKey) -> PathSet {`

The predicted region an ordinary dispatch of `key` takes.

**Read off the fold, not restated.** It answered `RepoWide` for every
task while the fixture's entries freeze `src/{id}/` hints, so every
dispatch this scaffold emitted recorded a region the fold did not
derive — the exact disagreement `check_dispatched` now refuses. A
literal here would be a second derivation of the run's own rule, which
is what let the two drift in the first place.

## `impl Run` › `pub(super) fn observed(&self, site: EffectSiteId, phase: HookPhase) -> bool {`

Whether the harness saw `site` at `phase`.

## `impl Run` › `pub(super) fn order_of(&self, site: EffectSiteId, phase: HookPhase) -> Option<usize> {`

Where `(site, phase)` **first** appears on the timeline, or `None` if
nothing drove it.

This is how an ordering clause over a prefix that runs once is asserted:
every family records onto one timeline, so an append and a
`git worktree add` are two positions in one list.

## `impl Run` › `pub(super) fn must_order_of(&self, site: EffectSiteId, phase: HookPhase) -> usize {`

[`Self::order_of`], or a panic naming what never ran.

## `impl Run` › `pub(super) fn mark(&self) -> usize {`

Everything that has happened so far, as a fence.

A clause about a *second* occurrence — O24's retry runs in a generation
whose dispatch already drove both of its sites — is asserted from a mark
taken before the step, so the positions compared are the step's own.

## `impl Run` › `pub(super) fn order_after(&self, mark: usize, site: EffectSiteId, phase: HookPhase) -> usize {`

The first position at or after `mark` at which `(site, phase)` ran.

## `impl Run` › `pub(super) fn count_after(&self, mark: usize, site: EffectSiteId, phase: HookPhase) -> usize {`

How many times `(site, phase)` ran at or after `mark`.

## `impl Run` › `pub(super) fn arm(&mut self, site: EffectSiteId, phase: HookPhase, injection: Injection) {`

Arm `injection` at a phase of a git funnel site.

## `impl Run` › `pub(super) fn arm_point(`

Arm a parent-side sub-effect point on the shared harness, which is where
a point genuinely belongs.

## `impl Run` › `pub(super) fn task_state(&self, key: TaskKey) -> TaskState {`

One task's state.

## `impl Run` › `pub(super) fn adopt(root: PathBuf) -> Self {`

Re-open the run a kill child left behind, and replay its log.

This is `recover.rs`'s shape reduced to what `T-DISPATCH` and
`T-ATTEMPT` need: open the log through `Event.OpenLog` (which truncates
a torn tail), parse the surviving bytes, and replay them. It is
deliberately **not** a call into the recovery order — that order is
another lane's and this fixture may not be a second implementation of
it. What it is is the smallest thing that makes the child's durable log
readable, so an assertion can be about the log rather than about a
message the child never got to send.

## `pub(super) fn adopt(root: PathBuf) -> Self` › `let paths =`

`RunPaths`'s own doc: "Callers do this once at run start;
every accessor below assumes it has happened." The scaffold
was handing out paths without it, so a review's transcript
write failed into `unavailable_after_error` and the pass was
recorded as an OUTAGE — which spends no attempt. A fixture
that skips a documented precondition does not fail loudly;
it produces a plausible wrong answer.

## `impl Run` › `pub(super) fn hand_off(&self, dir: &Path) {`

Tell the parent where this child's repository is.

Written **before** anything is armed, so a child that dies at its first
site still hands over a readable pointer. The parent has no other way to
learn it: the child's fixture root is named with a fresh ULID of the child's
own, which nothing in the parent can compute.

## `impl Run` › `pub(super) fn dispatch(&mut self, key: TaskKey, generation: u32) -> Dispatched {`

An ordinary dispatch of `key` at this fixture's head.

## `impl Run` › `pub(super) fn try_dispatch(`

[`Self::dispatch`], keeping the error.

Obligation (3) is discharged against a ledger of this fixture's own:
`dispatch` emits without holding one, so the failure carries the
obligation out, and something has to be the caller. In production that
is `TopologyRun`, which owns the run's ledger.

## `impl Run` › `pub(super) fn spawn_repair(&mut self, root: TaskKey) -> TaskKey {`

Register a repair of `root`, the way a merge rejection will.

PR9 owns the production producer; what this needs to be is an entry the
**fold** accepts, so that a repair dispatch is checked by the same rules
a real one will be. Everything but the identity is cloned from the root's
own entry — ladder, reviews, allowed agents — because every one of those
is a value `check_spawn` compares against the run header, and inventing
them here would be inventing a way to fail.

## `pub(super) const RETAINED_SESSION: &str = "session-01SCAFFOLD";`

The session a retained settlement holds, so a retry has one to resume.

## `impl Run` › `pub(super) fn binding(&self, key: TaskKey, rung: u32) -> RungBinding {`

The binding rung `rung` of `key`'s frozen ladder gives.

Read out of the registry rather than written here, because
`check_attempt_started` compares `attempt_started`'s binding against
exactly this value (INV-19) and a fixture that spelled it out would be
asserting the fold against a literal instead of against the ladder.

## `impl Run` › `pub(super) fn attempt_plan(&self, key: TaskKey, attempt: u32) -> AttemptPlan {`

One attempt of `key`: a worker, one gate, two reviewers.

Two reviewers rather than one, because
`decisions.workspace_candidates.snapshots` requires "one **fresh**
snapshot per reviewer, never reused across roles or attempts", and a
single reviewer cannot distinguish a fresh snapshot from a reused one.

## `pub(super) fn attempt_plan(&self, key: TaskKey, attempt: u32) -> AttemptPlan {` › `gates: vec![{`

Through the production assembler, not invented here. A fixture
that built its own `(command, timeout)` pair would be a second
derivation of the one thing `ShellGate::command` exists to be —
the `frozen_binding` precedent, where a fixture repeating a
production composition kept a fifth copy of it alive.

## `pub(super) fn attempt_plan(&self, key: TaskKey, attempt: u32) -> AttemptPlan {` › `reviewers: vec![`

Identity and policy, no command: the shared review machinery
builds one per invocation, because a re-ask's prompt is not the
first pass's. A fixture carrying a pre-built command would be a
pass shape production never builds.

## `impl Run` › `pub(super) fn review_inputs(&self) -> super::attempt::ReviewInputs {`

What every review pass of one scaffold attempt reads.

Owned fixture data rather than a plan shape invented here: a review that
could not be produced from these inputs would be a pass shape production
never builds.

## `impl Run` › `pub(super) fn retain(&mut self, key: TaskKey, generation: GenerationId, attempt: u32) {`

Settle the in-flight attempt as `Retained`, so the generation becomes
`RetainedIdle` and a same-session retry is admissible.

`settle.rs` owns this transition in production; what is needed here is
only the *state*, so the event is emitted through the same fold-checked
emitter every other event uses and is refused if it is not a transition
the fold allows.

## `pub(super) fn retain(&mut self, key: TaskKey, generation: GenerationId, attempt: u32) {` › `failure: Some(crate::events::FailureRecord {`

**A retained attempt did not succeed.**
`settle::settle_failed` is the only producer of a
`Retained` settlement and it is reached on the
failure path, so production's record always
carries this. This fixture recorded `failure:
None` with no reviews — a record every other door
in the fold calls *successful* — which is the
shape `check_attempt_finished`'s retained arm now
refuses.

## `const HANDOFF: &str = "fixture-root";`

The file a kill child writes its repository root into.

## `pub(super) const KILL_CHILD_BOUND: Duration = Duration::from_secs(120);`

The deadline a topology kill child is given. `kill_child_and_adopt`, the kill witnesses in
`recover/tests.rs`, the informational-append and open-log witnesses in `emit/tests.rs` and the
creation witnesses of rows 103 and 106 in `create/tests.rs` hand it to `run_kill_child_within`. It is
the finalization matrix's 120 seconds, which a loaded Windows guest has needed for a creation kill
child.
A child still running at the bound is ended there, and the witness fails naming its cell rather than
judging what the child left. `src/engine/tests.rs` cannot name this `#[cfg(test)]` module, so its
two launches carry a constant of their own with the same value.

## `pub(super) fn kill_dir(tag: &str) -> crate::rundir::scratch_tree::ScratchTree {`

The directory a kill test hands its child, acquired through the scratch-tree token and returned
as the guard, so it is reclaimed when the parent's test ends. It was
`temp_dir()/upstroke-topo-<tag>-<pid>-<ordinal>`, unique to a call within one process and removed
by nobody (`PR7-SCRATCH-FIXTURE-LEAK`).

## `pub(super) fn child_temporary_of(dir: &Path) -> [(&'static str, &std::ffi::OsStr); 3] {`

The environment that makes `dir` a child test process's temporary directory: `TMPDIR`, and the `TMP` and
`TEMP` Windows reads. For PR11's children (the coordinator's kill children and its two-process
containers' children), which `kill_child_and_adopt_in_a_scratch_tree` does not launch: a parent holds a
`kill_dir` guard, hands its path to the child through this, and drops the guard after the child has
ended and the run it left has been adopted and dropped, so what the child made under `temp_dir()` — its
fixture, and the neutral Git configuration every process that runs the fixture's Git writes once
(`fixture::neutral_git_config`) — is reclaimed with it. Until phase 7 those children wrote into the
suite's own temporary directory and nothing removed their configurations: 23 directories per suite run
(the PR11 record, §8, phase 7's residue). The nesting lengthens every path under such a child's fixture
by the guard's name, so the parents keep that name short: the record's phase-7 arithmetic bounds the
longest checkout `.git` path at 207 characters on the CI guest, 13 under Git for Windows' 220.

## `pub(super) fn kill_child_and_adopt(test: &str, dir: &Path, site: &str) -> Run {`

Run `test` as a child that must die, and adopt the run it left behind.

The `unreachable!` inside the child is what fails the test when an
injection stops killing; this end asserts the other half — that the process
really did not exit successfully. Both are needed: a child that returned
early would satisfy neither, and a child that panicked would satisfy only
this one.

The child is given `KILL_CHILD_BOUND` through
`run_kill_child_within`: a child that wedges instead of dying is killed and
reaped at the bound, and the test fails naming the site and the child, not
at whatever outer timeout would otherwise end the suite (#292's review round
2, `standards/12`'s bounded waits).

The launch and the checks are `launch_the_kill_child_and_adopt`'s, which this
calls with no environment beyond the directory and the site.

## `pub(super) fn kill_child_and_adopt_in_a_scratch_tree(`

`kill_child_and_adopt` over a directory the witness owns, for a witness that
must leave nothing behind (#292's review round 7). It acquires a
`rundir::scratch_tree` tree in the temporary directory, hands it to the child
as the handoff directory **and** as its temporary directory (`TMPDIR`, and the
`TMP` and `TEMP` Windows reads), and returns the guard with the adopted run.
So everything the child makes in its temporary directory lies inside the tree:
the fixture it builds and hands off, and the neutral Git configuration its Git
calls write once per process (`fixture::neutral_git_config`), which the child's
abort would otherwise leave behind. The witness binds the guard before the run
(`let (_handoff, mut run) = ...`), so the run drops first, removing the fixture
it adopted, and the guard then reclaims the tree, when the witness returns and
when it unwinds. A reclaim that fails on the return fails the test naming the
root; one that fails while the test unwinds is reported without a second panic
(`rundir::scratch_tree`'s own tests witness both). `kill_dir` and
`kill_child_and_adopt`, which the older kill witnesses use, are left as they
were: their handoff directory is named for the process and nothing removes it.

The tree's tag is `kill`, the same for every site, and short on purpose. The
nesting lengthens every path under the child's fixture by the tree's name, and
Git for Windows does not add a worktree whose `.git` path is longer than 220
characters: `git worktree add` exits 128 with `fatal: '$GIT_DIR' too big`
(measured on the Windows guest, with Git 2.50.1, the version CI's
`test (winguest)` runs, and `core.longpaths` unset: 220 added, 221 refused).
With the site in the tag, that leg failed at `2780e0e4` on the
`after_snapshot_intent` child, which ended 101 instead of aborting; the launch
discards the child's standard error, so that is all CI shows. On the guest,
under the same temporary directory, `C:\Users\Administrator\AppData\Local\Temp`,
and with that standard error kept by a diagnostic build, the child had
panicked in its dispatch: Git refused its task worktree, whose `.git` path was
223 characters (#292's review round 8).

## `fn launch_the_kill_child_and_adopt(`

The launch, the abort check and the adoption both entry points share: the child
gets the handoff directory and the site, and whatever `temporary` adds.

## `pub(super) fn kill_child_environment() -> (PathBuf, String) {`

The directory and site a kill child is given.

## `pub(super) const OUTCOME: RunOutcome = RunOutcome::Complete;`

The run outcome a run-end closure records in these tests.

## `pub(super) struct Ran {` › `pub(super) head_at_spawn: Option<String>,`

The commit the workspace's HEAD named when the process was spawned —
what a gate or reviewer actually looked at — or `None` when the
workspace is not a checkout.

## `impl super::attempt::ReviewPasses for ScaffoldReviews` › `fn run(`

**The double cannot ignore the workspace it was handed.** It runs a
process through the Runner in `cx.workspace` — as `run_review` runs the
adapter's CLI there — before returning its scripted verdict, so the
recording runner sees each reviewer's checkout, its HEAD at spawn and its
path, and the isolation oracles can assert them. The cover review of
`8a5f59e8` pointed the integration reviewers' `ReviewCx.workspace` into
the staging worktree, kept snapshot creation and cleanup, and passed every
engine-topology test while `verification_isolation` was violated, because
this double and the driven loop's both read only the profile
(`PR8-R4-REVIEW-ORACLE`), the third time a review double ignoring its
workspace had hidden a defect on that branch. A Runner error is answered
the way `run_review` answers it: a cancelled error or an unresolved fate
propagates, anything else is an unavailable review.

## `pub(super) enum VerifyReview {`


What the scaffold's integration verification decides, so a test drives a
stale_clean pass, a code rejection, a human-required park, or an outage
without a real reviewer.

## `fn verify(` › `match judge.judge(&Subject {`

The same mapping production makes (`run.rs`): a Runner that could
not run a gate is an outage with a terminal, not an error.

## `impl Run {` › `fn begin(run: &mut Self, started: RunStarted4) {`

Emit `run_started` and create the integration ref (P8), the two steps
every constructor shares.

## `impl Run {` › `pub(super) fn started_with_max_defers(tag: &str, max_defers: u32) -> Self {`

[`Self::started`] with a chosen `max_defers`, for the deferral tests.

## `impl Run {` › `pub(super) fn wake_deferred(&mut self) {`

Wake every verification-deferred candidate: `defer_wait_elapsed`.

## `impl Run {` › `pub(super) fn queue_candidate(`

Carry `key` from its first dispatch to a queued candidate through the
real candidate sequence: a worker edit, the capture, the commit, the
pin, `candidate_prepared`, the candidates ref, `task_candidate_created`,
and the scrub. The candidate's base is the run's own.

## `impl Run {` › `pub(super) fn queue_candidate_editing(`

Queue a candidate whose worker edits exactly `path` to `content`, so a
later candidate editing the same path conflicts with it, and one editing
it to the same content is already present.

## `impl Run {` › `pub(super) fn integration_ref(&self) -> GitRef {`

The run's integration ref, as `run_started` recorded it.

## `impl Run {` › `pub(super) fn head(&self) -> Option<String> {`

What the integration ref names right now.

## `impl Run {` › `pub(super) fn replay_twice_equal(&self) {`

Replay the durable log twice and check both replays agree with the
live fold.

## `impl ReviewPasses for ScaffoldReviews {` › `let never_started = matches!(error.fate, crate::error::ProcessFate::NeverStarted);`

As `run_review` does: the double reports what the Runner established about the
process, so it cannot hide the outage attribution INV-23 names. A double that
answered `false` here would make
`recover::tests::a_reviewer_whose_process_never_started_is_a_runner_spawn_failure`
unwritable and the arm untestable — the same shape as the review doubles that
ignored the workspace they were handed and hid the isolation defect of the
round before.

## `fn run_started_of(`

`run_started` for a plan, a width and a review shape: `max_parallel` is the
width the coordinator's tests run at, and `second_opinion` records the
second review pass for every task, so a plan whose reviewers number two
freezes the passes its judgements will record.

## `pub(super) type Responder =`

An invocation's result as a function of its request alone.

## `pub(super) struct RecordingRunner` › `respond: Mutex<Option<Responder>>,`

When set, every request is answered by the responder instead of the queued
codes — held or not — so what a process returns never depends on the order in
which concurrent pipelines reached the runner.

## `impl std::fmt::Debug for RecordingRunner {`

By hand, because a responder is a closure and has no `Debug`.

## `impl RecordingRunner` › `pub(super) fn answering(self, respond: Responder) -> Self {`

Answer every request with `respond`.

## `impl RecordingRunner` › `pub(super) fn await_held(&self, invocation: &InvocationId, within: Duration) -> bool {`

Block until `invocation` is held with no result delivered, or `within` passes.

## `impl RecordingRunner` › `pub(super) fn release(`

Deliver a held invocation the result its responder gives its recorded request
(an exit 0 when no responder is set): the step a deterministic schedule takes
to let one chosen invocation finish.

## `pub(super) fn exited(code: i32, stdout: String) -> ProcessOutput {`

A process that exited `code` having printed `stdout`.

## `pub(super) const PASSING_VERDICT: &str =`

A reviewer's passing verdict, as the answering adapter echoes it from its
process's output; `WORKER_QUESTION` is a worker's question, in the marker
form the worker adapter parses.

## `impl AnsweringAdapter` › `pub(super) const fn echoing(id: &'static str) -> Self {`

An adapter whose outcome's `detail` is its process's output, so the responder,
not the adapter, decides what a worker or reviewer said.

## `impl ScaffoldAdapters` › `pub(super) const fn echoing() -> Self {`

The same two agents, each echoing its process.

## `impl AnsweringAdapter` › `pub(super) const fn probing(id: &'static str) -> Self {`

A passing adapter that also answers the pre-flight: its `probe` runs `<id> --version` through the
runner it is handed, under the agent's own probe-role `InvocationId` (`agent::probe_request`, ordinal
0), and reports fixed capabilities when the process exits 0. It is how a width-3 run resumed through
the frozen recovery order sends its agent probes through a real `RunPreflight` (phase 6's
`runtime_pool_agent_probes_take_and_release_their_pair_at_preflight_before_admission`); every other
adapter of this file still refuses to probe.

## `impl crate::agent::AgentAdapter for AnsweringAdapter` › `fn probe(&self, runner: &dyn Runner) -> Result<crate::agent::Caps, UpstrokeError> {`

An adapter built without `probing` fails the assertion that it is one: the scaffold's attempts never
pre-flight, and a probe reaching such an adapter is a fixture wired to the wrong boundary.

## `impl ScaffoldAdapters` › `pub(super) const fn probing() -> Self {`

The same two agents, each answering its pre-flight probe.

## `pub(super) struct NoSleep;`

A sleeper that returns at once, so a backoff costs a test nothing.

## `pub(super) struct WideTask {`

One task of a width-N plan: its id, its conflict hints, what it depends on and
the file its worker writes.

## `pub(super) fn wide_plan(tasks: &[WideTask]) -> Plan {`

The plan the tasks describe, built directly rather than parsed: each task's
dependencies and path hints as given, and one acceptance line each.

## `pub(super) struct WidePlans {`

The attempt and verification plans a width-N run uses: how many gates and
reviewers each attempt and each verification has, and the pool its agent
slots come from. `panic_verifying` makes every verification's plan panic, on the
verification pipeline's thread, as its body starts: a fatal completion the
coordinator must classify as it arrives (round R1, the early review's
`R1-CONC-2`). `pools` names a pool per agent, overriding `pool` for the agents it
lists — an entry of `None` gives that agent no pool while the others keep theirs —
which is how phase 6's runtime pool tests give two agents pools of their own, or one
agent none beside a pooled one.

## `impl WidePlans` › `fn pool_of(&self, agent: &str) -> Option<String> {`

The pool an agent's pairs name: its `pools` entry when it has one, else `pool`. The
worker's (`plan`, and `pool_for`, which the retry path asks) and every reviewer's
(`reviewer_plans`, whose profile carries it) come from here, so one run's attempts,
retries and verifications agree on each agent's pool.

## `pub(super) fn wide_responder(tasks: &[WideTask], failing_gates: &[(u32, u32)]) -> Responder {`

Every invocation's result as a function of its request, so an outcome never
depends on which pipeline started first: a worker writes its task's file into
the worktree it runs in and exits 0; a gate exits 0 unless its (task, attempt)
is listed as failing; every other process exits 0.

## `pub(super) fn wide_responder_asking(`

[`wide_responder`], with the workers of the listed task keys asking a
question instead of working.

## `pub(super) struct WideEnv {`

Everything a width-N run's seams borrow or share: the answer source, the
runner and adapters behind `Arc`s a coordinator's pipelines share, the plans,
the paths, the log and the fixture.

## `pub(super) struct Wide {`

A started width-N run and its environment.

## `impl Wide` › `pub(super) fn started_with(`

A run started as `create` would leave it: the run lock and worktree lease
taken, `run_started` appended and folded, the integration ref created, and a
[`super::run::TopologyRun`] resumed over the handle — so the coordinator's tests
drive the real transitions from the first selection on.

## `impl WideEnv` › `pub(super) fn pipelines_limited(`

The coordinator's pipeline seams over the same runner, adapters and plans,
each pipeline's hooks recording into the one harness, with the slot limits
defaulted from the width or given exactly.

## `pub(super) enum SlotLimitsOf {`

How a test chooses the coordinator's slot limits.

## `impl Wide` › `pub(super) fn started_under(`

[`Wide::started_with`] under a spend ceiling, for the budget stop's tests.

## `pub(super) const DURABLE_INCARNATION: &str = "inc-1";`

The incarnation a durable run is created under, and the one the workspace
fixture's manager carries.

## `pub(super) struct Certifying;`

A pre-flight that certifies every recorded runner: a durable run's resume
probes nothing, since the scaffold runner is not a process.

## `fn await_release_of_the_previous_process(public: &Path) {`

Before a resume takes the run's locks, wait — at most
`PREVIOUS_PROCESS_RELEASE_BOUND` — until no cleanup hold on the run's public
directory is observed. A child another test thread forked while the first
process held its lock can keep an inherited descriptor for about a millisecond;
the recovery trunks' tests wait the same way before they resume.

## `fn durable_paths(fixture: &Fixture) -> crate::rundir::RunPaths {`

The run directories run creation lays out: the public half beside the
repository, the private half at `<private root>/runs/<run id>`.

## `impl Wide` › `pub(super) fn durable(`

A width-N run the frozen recovery order can resume
([`Wide::durable_under`] without a ceiling).

## `impl Wide` › `pub(super) fn durable_under(`

[`Wide::durable_with`] under the durable incarnation and the recorded host
runner, with a spend ceiling.

## `impl Wide` › `pub(super) fn durable_contained(`

A durable run that recorded the **container** runner ([`container_policy`]),
under the incarnation given, for the tests whose pipelines run containers. The
incarnation must be a container-name component — `[0-9A-Za-z_]`, which the
durable incarnation `inc-1` is not — so these runs take a ULID-shaped one.

## `impl Wide` › `fn durable_with(`

What run creation leaves, without its probes: the log at the public
directory's `events.jsonl` holding `run_started`, the private half's owner and
commit records agreeing with it, one run id and incarnation throughout, and the
integration ref at the base; the run and worktree locks are taken before any
ref is written, as [`Wide::started_with`] takes them. The incarnation and the
recorded runner come from the `DurableIdentity`; an incarnation other than the
workspace fixture's re-derives the manager under it. The phase-3 fixture is not
this one: its log and private directory are laid out for driving only, and no
recovery could adopt them (the working record's R-AL).

## `pub(super) struct DurableIdentity {`

Who a durable run is recorded as: its incarnation, and the runner policy it
records (the host's when `None`).

## `impl Wide` › `pub(super) fn resume(`

Drop this process's run — its locks and fold with it — and hand the directory to
[`WideEnv::resume`].

## `impl WideEnv` › `pub(super) fn adopted(`

The environment over a durable run a killed child left, rebuilt from its root:
the parent never trusts what the child believed about its own state.

## `impl WideEnv` › `pub(super) fn resume(`

The next process over an empty fake container runtime, a fake owner liveness
that calls nobody live, and [`Certifying`], through [`WideEnv::resume_over`].

## `impl WideEnv` › `pub(super) fn resume_over(`

[`WideEnv::try_resume_over`], dropping the environment on a refusal.

## `impl WideEnv` › `pub(super) fn try_resume_over(`

The next process: disarm the harness, wait for the previous process's release
(unless told not to), derive a manager under the new incarnation, and run the
frozen `recover::run_recovery_order` with the seams given — a container runtime,
an owner liveness and a pre-flight — and a disposable Git view and the manager as
the ref funnel; then build the run from the handle it returns, with a fresh
runner and the given ceiling. A refusal hands the environment back beside the
error, so a test can observe what the refused process left and try again over
the same fixture.

## `pub(super) struct ResumingOver<'a> {`

The seams a resume censuses and certifies through: the shared runtime of a
two-process test and the production `LockProbe`, or the in-process fakes.
`awaits_release: false` skips the wait for a previous process's cleanup hold,
for the test that must observe the next coordinator refused over one.

## `pub(super) fn container_policy() -> RunnerPolicy {`

The container runner a contained run records: one image by reference, id and
digest, and a credential volume per agent — the worker's and the reviewer's.

## `pub(super) fn container_host() -> crate::runner::container::FakeRuntime {`

The shared fake container runtime of a test, holding the recorded image and
both credential volumes. Every process of the test reaches this one fake: the
parent through handles (`acting_as`), each child through its daemon.

## `pub(super) fn container_runner(`

The production `ContainerRunner` for a run identity over a runtime, with a
disposable Git view and the supervision poll given. No runner-level observer is
installed, since one would take the runner's `hooks` lock across every
invocation and serialize the pipelines.

## `pub(super) fn exiting() -> crate::runner::container::StartPolicy {`

Every container this handle starts exits at once with code 0: a probe that
succeeds, a foreign command's one invocation.

## `pub(super) fn exiting_probes() -> crate::runner::container::StartPolicy {`

Probes exit at once and everything else holds: a resuming child passes its
pre-flight and then runs its pipelines' containers until it is killed.

## `fn played_by(`

A container's process is the scaffold double's invocation of the request that
created it, found by the invocation its labels name. So the deterministic
scheduler holds and releases containers exactly as it holds and releases the
double's invocations, and the container exits with the double's output.

## `pub(super) struct Contained {`

A coordinator's runner of containers: the production `ContainerRunner`, whose
starts run the double ([`played_by`]); the decorator only records each request
before the runner plans it.

## `impl WideEnv` › `pub(super) fn identity(`

The run identity a durable run's containers carry: its private root, run id,
public run directory, the incarnation given, and its repository key.

## `impl WideEnv` › `pub(super) fn contained(`

A [`Contained`] runner for this environment's double, over a handle of the
shared runtime named after the incarnation.

## `impl WideEnv` › `pub(super) fn pipelines_over(`

[`WideEnv::pipelines`] with another runner.

## `impl WideEnv` › `pub(super) fn seams_over<'a>(&'a self, runner: &'a dyn Runner) -> super::run::RunSeams<'a> {`

[`WideEnv::seams`] with another runner.

## `pub(super) struct Served {`

The parent's side of a second process: the linked child, the daemon thread
serving its container requests against a handle of the shared runtime, and the
events it reports. Dropping it kills the child and joins the daemon, however
the test ends.

## `impl Served` › `pub(super) fn event(&self, what: &str) -> serde_json::Value {`

The child's next report; a child that sends none within the link's bound fails
the test with its stderr.

## `impl Served` › `pub(super) fn exited(&self, what: &str) -> std::process::ExitStatus {`

Wait for a child that is meant to finish.

## `pub(super) struct ParentSide {`

The child's side: attach once, report events, and reach the parent's container
runtime ([`crate::runner::container::LinkedRuntime`]).
