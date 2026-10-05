# `src/engine/topology/attempt.rs`

Extended notes for [`src/engine/topology/attempt.rs`](../../../../src/engine/topology/attempt.rs).

The code is the authority for what it does; this file is the whole of its prose, moved out of
the source verbatim. Each section is headed by the line of code the comment sat above, spelled
as it is in the source, so the heading is the grep string that finds the code.

## Module

The attempt: `attempt_started` → worker → capture → snapshots → judgement.

Five ordering clauses, and each of them is a statement about what may exist
on disk when a coordinator dies. That is why they are clauses rather than
style: `T-ATTEMPT` tables four sub-prefixes — (a) worker running, (b') a
capture command interrupted, (b) capture done, (c) an ephemeral snapshot
commit with no worktree, (d) a snapshot worktree with gates running — and
each one is a **different** durable residue with a **different** tabled
recovery.

* **O23 — `attempt_started` before spawn.** The event that says an attempt
  is in flight is durable before the process that spends money exists.
  `T-ATTEMPT`'s `authoritative_state` is "unknown spend, nothing judged", and
  unknown spend is recoverable only because the event precedes the spend.
* **O24 — retry: reservation, worktree verification, `attempt_started`
  (retry), spawn.** A clause with two owners, and this module owns the
  second half only. The reservation and the verification are
  [`super::settle::retry`]'s: it takes the `{pipeline}` reservation, runs
  O22's `Worktree.Verify` against the retained cumulative tree, and on a
  failure cancels that reservation and closes the generation with
  `generation_closed{WorktreeMissing}` — which is what INV-06 requires,
  because a `RetainedIdle` worktree "is never recreated". What it hands
  back is the `attempt_started(retry)` it authorized, and
  [`AttemptContext::start`] appends exactly that event and spawns.

  There is deliberately **no retry entry point on this side**. One would
  have to re-observe a worktree `settle::retry` has already passed, and a
  refusal from that second look would strand the reservation the first one
  took: the branch that cancels it is `settle::retry`'s failure branch, and
  that branch was not taken. One observation, one recovery, one owner each.
* **O25 — capture before snapshots.** `Object.CandidateStage` then
  `Object.CandidateWriteTree`, whose objects are referenced by the task
  worktree's index (R9). A snapshot taken before the capture would be a
  snapshot of a tree that does not exist yet.
* **O26 — ephemeral snapshot commit before the snapshot intent, intent
  before the snapshot add.** The commit is unreferenced (R27) when it is
  written, so a death between it and the intent leaves an object that is
  Git's and nothing else's. [`WorkspaceManager::add_snapshot`] performs the
  three in that order.
* **O27 — gates and reviews before commit-tree.** The candidate commit is
  `candidate.rs`'s (O28–O31); what this module owes is that every judgement
  is complete, and complete **on fresh exact snapshots**, before that module
  is entered.

### Where the attempt path hands off

At the judgement, and **not** at a settlement. This module appends nothing:
a failed attempt settles on `attempt_finished` and a successful one on
`candidate_prepared`, which is the sole successful settlement for a
candidate-producing attempt and is what promotes the generation.

**This described the other shape** — `check_candidate_prepared` requiring
`Promoting`, and an `attempt_finished(succeeded)` between the pin and
`candidate_prepared` to produce it. That requirement *forced* the dual
settlement `design/26_design_merge_queue_protocol.md` §26 forbids,
and the 2026-08-27 CONFORM ruling reversed it: the fold now requires the
generation to be **in flight**, because this event settles it.

`T-CAND-OBJ`'s window is "attempt unsettled" across both the commit object
and the pin, which is the same statement from the fault matrix's side, and it
now runs all the way to `candidate_prepared`. What this module produces is
the [`Judgement`] that `candidate.rs` gates its sequence on.

### Every process here goes through the run's `&dyn Runner`

Worker, gates and reviewers alike, each carrying the [`InvocationId`] that
[`AttemptIdentities`] assigns. `permits.agent_pool_slots` then splits them:
"every agent CLI invocation acquires its atomic `{agent, pool?}` pair:
worker, review_pass, review_reask … and agent probe; **gate invocations and
the shell probe acquire no slot**". [`is_slotted`] is the single reading of
that sentence and this module never re-decides it.

Since PR11's broker the review passes cross the Runner through the broker's
registering boundary too ([`Judge::judge`]), so every slotted process of an
attempt — the worker, each review pass, each re-ask — is registered with its
pair before it starts and holds it until it ends. Before that a review pass
reached the Runner raw and was entered in the ledger only after it returned
(`PR7-R3-ATTEMPT-002-REVIEWERS-TAKE-NO-SLOT`).

## `pub struct ReviewerPlan {`

---------------------------------------------------------------------------
What an attempt is asked to run
---------------------------------------------------------------------------

## `pub struct ReviewerPlan {`

One reviewer of one attempt.

A reviewer is an agent CLI, so it is slotted and gets its own **fresh**
snapshot: `decisions.workspace_candidates.snapshots` is "one snapshot for
the gate set and one fresh snapshot per reviewer, never reused across roles
or attempts".

## `pub struct ReviewerPlan` › `pub agent: AgentId,`

The agent whose CLI this pass runs.

## `pub struct ReviewerPlan` › `pub profile: WorkerProfile,`

The routing decision this pass was bound with: model, effort, pool.

`AttemptRecord` requires the model as a `String` and not an `Option`,
so a plan that carried only a command could not produce the record the
run must write.

## `pub struct ReviewerPlan` › `pub lens: review::Lens,`

Which review this is — what distinguishes it from the other passes.

## `pub struct ReviewerPlan` › `pub preflight_cli_version: Option<String>,`

What pre-flight certified this CLI as, where it certified one.

## `pub struct ReviewerPlan` › `pub timeout: std::time::Duration,`

How long one invocation may take.

## `pub struct ReviewInputs {`

What every review pass of one attempt reads.

Owned, and on the plan rather than the context, because it is **data the
caller decided** — the same reason `worker: CommandSpec` is on the plan. The
context holds seams; the plan holds what this attempt is.

## `pub struct ReviewInputs` › `pub title: String,`

The task under review, as the prompt quotes it.

## `pub struct ReviewInputs` › `pub body: String,`

Its body, which may be empty.

## `pub struct ReviewInputs` › `pub acceptance: Vec<String>,`

Its acceptance criteria.

## `pub struct ReviewInputs` › `pub diff: String,`

The diff being judged.

## `pub struct ReviewInputs` › `pub artifacts: Vec<(String, String)>,`

Named artifacts the prompt wires to real files.

## `pub struct ReviewInputs` › `pub decisions: Vec<String>,`

Operator decisions the judge must honour, as the worker was given them.

## `pub struct ReviewInputs` › `pub stem: String,`

The per-attempt file stem transcripts and settings are named from.

## `pub struct PlanRequest<'a> {`

What only the driver knows about the attempt a plan is being built for.

The frozen entry travels whole rather than narrowed. Narrowing earns its
keep where a consumer reads three fields of fifteen — it is why
[`crate::review::ReviewBindings`] exists — and the plan assembler reads the
spec's five prompt fields, the ladder's effort, and all three review
bindings. A subject that wide is the entry with a second name.

The reference is short-lived by construction: the driver reads it out of the
fold it owns, and the plan that comes back is an owned value, so the borrow
ends before the append the plan authorizes.

## `pub struct PlanRequest<'a>` › `pub key: crate::topology::registry::TaskKey,`

The task.

## `pub struct PlanRequest<'a>` › `#[allow(dead_code)]` (trailing)

never called at 610106b; see `PR7-NARROWED-SURFACE-19-UNCALLED` (§2)

## `pub struct PlanRequest<'a>` › `pub entry: &'a crate::topology::registry::TaskEntry,`

Its frozen registration — spec, ladder, review bindings.

## `pub struct PlanRequest<'a>` › `pub attempt: AttemptNumber,`

Which attempt of this generation.

## `pub struct PlanRequest<'a>` › `pub rung: u32,`

Index into the frozen ladder.

## `pub struct PlanRequest<'a>` › `pub binding: RungBinding,`

The binding that rung resolved to, effort included.

## `pub struct PlanRequest<'a>` › `pub workspace: &'a std::path::Path,`

The task worktree the worker runs in.

## `pub struct PlanRequest<'a>` › `pub resume_session: Option<SessionId>,`

The session a same-session retry resumes.

## `pub struct PlanRequest<'a>` › `pub feedback: Vec<crate::events::Feedback>,`

What the previous attempts failed on, oldest first.

§11.4: failure feedback goes back to the same rung, and an escalation
carries the accumulated feedback with it. This was always empty — the
plan hard-coded `retry: None` — so a retried worker was given the same
prompt as the first attempt and no reason to do anything differently.
The whole point of spending a second attempt is that it is informed.

## `pub struct PlanRequest<'a>` › `pub materialization_observed: Option<Materialization>,`

What a repair's worktree looked like when this attempt started.

## `pub struct InputsRequest<'a> {`

Where an [`AttemptPlan`] is assembled.

**A seam for the same reason [`ReviewPasses`] is one.** Building the plan
means building the worker's command, and
[`crate::engine::assembly::WorkerAssembly::command`] materializes the
permissions file that defines the attempt's sandbox — a write outside any
inventoried `RunDir` site, in a legacy funnel this module may not be
allowlisted into. So the topology module declares the plan it needs and
something outside the tree builds it.

It is also the honest shape. A plan needs the run's config — the gate set,
the worker allowance, the pool table, the probed CLI versions — and none of
that is in the event log the fold replays. The driver knows the attempt; the
implementation knows the run it is part of.

## `pub struct InputsRequest<'a>` › `pub entry: &'a crate::topology::registry::TaskEntry,`

The task under review, as the prompt quotes it.

## `pub struct InputsRequest<'a>` › `pub diff: String,`

The diff being judged, as the engine captured it.

## `pub trait AttemptPlans {`

Where an [`AttemptPlan`] and the review inputs beside it are assembled.

## `pub trait AttemptPlans` › `fn inputs(&self, request: &InputsRequest<'_>) -> Result<ReviewInputs, UpstrokeError>;`

What a reviewer is shown, beside the diff the driver captured.

Separate from [`Self::plan`] because it needs the *outcome* of the
attempt — a plan is built before the worker runs and this is built
after. Same implementation, because both read the run's config: the
artifacts directory a named artifact resolves against, and the operator
decisions the judge must honour.

### Errors

Whatever the assembler returns.

## `pub trait AttemptPlans` › `fn pool_for(&self, agent: &str) -> Option<String>;`

The capacity pool `agent` drains, where the run's table names one.

**Here rather than in the driver because the pool rule has one
production implementation** and this seam owns the frozen table it reads.
The dispatch arm gets the pool from the plan (`plan.pool`); the retry arm
appends `attempt_started` **before** its plan exists — `settle::retry`
produces the event and the plan is built after — so it needs the same
answer one step earlier. Resolving it in the driver would be a second
place that decides which pool an attempt drains, which is what
`a_reviewers_profile_is_accounted_for_at_both_callers` exists to forbid.

Sol's `R3-SEAMS-001`: the retry passed `pool: None`, so a resumed run's
`attempt_started` recorded no pool while the plan it then built resolved
one — the ledger and the plan disagreeing about the same attempt.

## `pub trait AttemptPlans` › `fn plan(&self, request: &PlanRequest<'_>) -> Result<AttemptPlan, UpstrokeError>;`

The plan for one attempt.

### Errors

Whatever the assembler returns: an agent with no registered adapter, a
permissions file that cannot be written, a binary that cannot be
resolved.

## `pub trait ReviewInputPolicy {`

Whether the staged evidence is the evidence a gate would see.

**The ladder's third cheap rung, and a seam for the reason
[`ReviewPasses`] is one.** `Workspace::review_input_problem_for_tree` is the
policy: it refuses a tree whose bytes a clean/smudge filter has transformed,
and a worktree still holding unstaged or dirty nested state behind an
unchanged gitlink. Either makes the diff describe something other than what
the gates run against, so neither can be reviewed completely.

It reads a [`crate::workspace::Workspace`], which is the legacy type; this
module holds a `WorkspaceManager` and a `Slot`. Re-deriving the policy on
the manager would be a second implementation of a refusal whose whole job is
to be exact, so the topology module declares what it needs and something
outside the tree performs it.

## `pub trait ReviewInputPolicy` › `fn problem(`

The problem with this tree as review input, or `None`.

### Errors

A Git or I/O error from the inspection.

## `pub trait ReviewPasses {`

Where a review pass is executed.

**A seam, and it is not optional.** `review::run_review` is on the effect
denylist — "UPSTROKE-WRAPPER: writes review transcripts through
`util::write_text`" — because it writes outside any inventoried `RunDir`
site. `decisions.effect_site_inventory.mechanism` (2) then says the legacy
allowlist "never contains a topology module", and this file is one, so the
escape is forbidden by name. `gates::run_all` is denied for the same reason,
which is why [`AttemptContext::judge`] runs gates through `gate_request`
itself rather than calling it.

Making the call legal directly would need a new `RunDirSite` variant for a
transcript write — there is none — in `topology::effects`, whose root
`src/topology/effects.rs` is the file `ff0490a` froze by name and whose site
enums are in its `sites.rs` child since that module was split. So the topology module declares what it
needs and something outside the tree performs it, exactly as
[`EventEmitter`], `Probes` and `IntegrationRefs` already do here.

**The authority is still `run_review`.** This is a seam over one
implementation, not a second one, and PR8's merge verification implements
the same trait rather than growing its own review path.

## `pub trait ReviewPasses` › `fn run(`

Run one review pass, re-asks and all.

### Errors

Whatever the review machinery returns. A reviewer that could not run is
**not** an error — it is a `ReviewResult::Unavailable`, which the ladder
defers rather than blaming on the implementer.

## `pub struct AttemptPlan {`

Everything one attempt executes, and the binding it executes under.

The binding, rung and pool are here rather than derived because
`attempt_started` records them and the fold checks them against the frozen
ladder: they are the attempt's execution identity (INV-19), and a module
that re-derived them would be a second authority for a value the registry
already froze.

## `pub struct AttemptPlan` › `pub attempt: AttemptNumber,`

Which attempt of this generation. A retry is a **new** number, which is
what makes its identities new.

## `pub struct AttemptPlan` › `pub rung: u32,`

Index into the frozen ladder.

## `pub struct AttemptPlan` › `pub binding: RungBinding,`

The binding this attempt used.

## `pub struct AttemptPlan` › `pub pool: Option<String>,`

The capacity pool, where the agent names one.

## `pub struct AttemptPlan` › `pub resume_session: Option<SessionId>,`

The session this attempt resumes. `Some` only for a same-session retry
in the incarnation that retained it.

## `pub struct AttemptPlan` › `pub materialization_observed: Option<Materialization>,`

What the repair's worktree looked like when this attempt started.
`Some` for a repair, `None` otherwise.

## `pub struct AttemptPlan` › `pub agent: AgentId,`

The agent the worker runs as.

## `pub struct AttemptPlan` › `pub session_resume: bool,`

Whether that agent's CLI can resume a session, as pre-flight probed it.

Half of `LadderState::resumable`; the other half is whether the attempt
actually returned a session id. Both are required, and the settlement
reads them together: a `Retained` generation with no session to resume
is a generation nothing can continue.

## `pub struct AttemptPlan` › `pub worker: CommandSpec,`

The worker's command.

## `pub struct AttemptPlan` › `pub worker_timeout: std::time::Duration,`

How long the worker may take.

## `pub struct AttemptPlan` › `pub gates: Vec<GatePlan>,`

The gate set, in order. Non-slotted, and run on one shared snapshot.

## `pub struct AttemptPlan` › `pub reviewers: Vec<ReviewerPlan>,`

The reviewers, in order. Slotted, and one fresh snapshot each.

## `pub struct GatePlan {`

One gate of the gate set.

No agent field, and that is the invariant rather than an omission: "a gate is
repository-controlled code and runs no agent CLI, so it takes no
`{agent, pool}` pair (R3)". [`gate_request`] is the one construction point
that says so.

## `pub struct GatePlan` › `pub name: String,`

The gate's configured name, which is what a failure reports and a human
reads. `gate {index}` named the position instead, so an operator had to
count the config to find out which gate rejected their task.

## `pub struct GatePlan` › `pub command: CommandSpec,`

What to run.

## `pub struct GatePlan` › `pub timeout: std::time::Duration,`

How long it may take.

## `pub struct AttemptRun {`

---------------------------------------------------------------------------
What an attempt produced
---------------------------------------------------------------------------

## `pub struct AttemptRun {`

The worker ran and the attempt is in flight.

## `pub struct AttemptRun` › `pub identities: AttemptIdentities,`

The identities every process of this attempt draws from.

## `pub struct AttemptRun` › `pub worker: ProcessOutput,`

What the worker process did.

## `pub struct Capture {`

The exact tree the capture wrote, and where it came from.

`decisions.workspace_candidates.candidate`: "capture the exact tree
(`Object.CandidateStage` then `Object.CandidateWriteTree`: the blob and tree
objects are referenced by the task worktree index, R9 …)". The tree id is the
whole product; the objects behind it are reachable only through that index
until something references the tree, which is what makes `T-ATTEMPT`'s
sub-prefix (b) a *distinct* durable state from (c).

## `pub struct Capture` › `pub tree: String,`

The tree `git write-tree` printed.

## `pub struct Capture` › `pub parent: String,`

The commit the tree is judged against, and the parent of every snapshot's
ephemeral commit.

## `pub struct Capture` › `pub unresolved: Vec<String>,`

What kept the capture from proceeding (`R9`): every path the index still
held unmerged that the worker's resolution manifest did not declare resolved
(or that could not be decoded), and the manifest's own problem, in
parentheses, when it had one. Read *before* staging: `git add -A` would
record the markers as the resolution. Non-empty means nothing was staged,
`tree` is the base's, and the assessment fails the attempt before any gate.

## `struct ResolutionPlan {`

The capture's reconciliation of the paths the worker's manifest governs with
what it declares: `staged`, the governed paths the manifest declared, each
with how, spelt as the index spells them; `refused`, every unmerged entry it
did not declare, every contradiction, plus the manifest's problem when it has
one. A plan with anything refused stages nothing.

## `fn plan_resolutions(`

Three inputs, two governed lists and the manifest: `unmerged`, the index's
conflicted entries, every one of which must be declared; `resolved`, the
entries a previous capture of this generation resolved and the index still
holds (`resolved_conflicts`), which a declaration may revise and silence
leaves to the ordinary `add -A`. No manifest: every unmerged entry refused,
every resolved entry left to the `add -A` — so an absent manifest in a
retained retry with nothing unmerged is an ordinary capture. A malformed one:
every governed path refused and the detail appended as one more entry, so
the worker is told the line. A parsed one, per governed path: declared one
way as the index spells it (`Declaration::names`, a path comparison), staged
that way — a resolved path declared `deleted` is the revision the retry
exists for; declared both ways, refused rather than guessed; declared one way
exactly and the other in another case (`names_in_another_case`, a spelling
that governs nothing itself), refused as a contradiction; declared only in
another case, refused naming the index's spelling; undeclared, refused if
unmerged and left to the `add -A` if resolved. A declaration naming a path in
neither list is nothing. (This note described the two-argument planner —
"no manifest: everything refused", "a declaration of a path that is not
unmerged is nothing" — until PR #249's fifth repair round, three rounds after
the resolved list was added; its manifest-contract review found the copy.)

## `fn captured_object_id(source: &str, value: String) -> Result<ObjectId, UpstrokeError> {`

One of the capture's recorded ids as an [`ObjectId`], or a Git error.

**§7: whose mistake is it.** These two values are not a caller's: the tree
is `git write-tree`'s own output and the parent is the base commit this run
recorded, both already object ids by the time they are held as strings. A
value here that is not one is the tool or the engine misbehaving, so it is
[`UpstrokeError::Git`] naming where the value came from, as
`add_snapshot` does for a `git commit-tree` line -- not
`UpstrokeError::Refused`, which says the caller offered something it should
not have and which `ObjectId::new`'s refusal would otherwise become here.

## `pub struct Assessment {`

What the ladder's **cheap rungs** said, before a gate or a reviewer ran.

`run_attempt`'s order is "outcome sanity → cheap static provenance → diff
classification → gates → review. Cheapest and most objective first", and
each rung is guarded by `failure.is_none()` — a worker that died is not
asked to pass gates. [`AttemptContext::judge`] implements the last two
rungs; this is the first ones, and without it the driver would hand an
empty diff to a frontier reviewer and then settle it as a success.

## `pub struct Assessment` › `pub outcome: crate::ir::Outcome,`

The adapter's parse of what the worker said about itself: session, cost,
usage, status. The attempt record is built from it.

## `pub struct Assessment` › `pub failure: Option<AttemptFailure>,`

What the cheap rungs concluded, if anything.

## `pub struct AttemptSite<'a> {`

Where one attempt runs, whichever branch put it there.

**A dispatch and a retry are the same attempt over different ground.** The
ready-dispatch branch opens a generation and gets a [`Dispatched`]; the
ready-retry branch continues one whose worktree already holds the previous
attempt's cumulative work, and no dispatch happened. Constructing a
`Dispatched` for the second would be a type asserting something false, so
the phases ask for the five facts they actually read instead.

## `pub struct AttemptSite<'a>` › `pub key: crate::topology::registry::TaskKey,`

The task.

## `pub struct AttemptSite<'a>` › `pub generation: crate::topology::events::GenerationId,`

The generation the attempt runs in.

## `pub struct AttemptSite<'a>` › `pub base: &'a crate::topology::events::CommitSha,`

The commit the worktree was created at.

## `pub struct AttemptSite<'a>` › `pub slot: &'a Slot,`

The worktree's slot.

## `pub struct AttemptSite<'a>` › `pub worktree: &'a std::path::Path,`

Its path on disk.

## `impl Dispatched` › `pub fn site(&self) -> AttemptSite<'_> {`

This dispatch, as the ground an attempt runs on.

## `pub struct Judging<'a> {`

What one attempt produced, for the judging that reads all three.

A bundle because they arrive together and are read together: `judge` needs
the identities to name its gate and review invocations, the tree to snapshot
from, and the cheap rungs' verdict to start its own ladder at. Passing them
singly put `judge` at eight arguments, which is a signal about the shape
rather than about the lint.

## `pub struct Judging<'a>` › `pub run: &'a AttemptRun,`

The identities and the worker process.

## `pub struct Judging<'a>` › `pub capture: &'a Capture,`

The exact tree and the commit it is judged against.

## `pub struct Judging<'a>` › `pub assessed: &'a Assessment,`

What outcome sanity and the diff already concluded.

## `pub struct Verdict {`

One process's verdict, as the runner reported it.

## `pub struct Verdict` › `pub output_limited: bool,`

Whether the Runner truncated the process's output.

§11.1 makes the 8-KiB tail the feedback a retry is given, so a truncated
log is a different claim from a short one and the ladder is entitled to
know which it has.

## `pub struct Verdict` › `pub timed_out: bool,`

Whether the Runner killed it on its timeout.

## `pub struct Verdict` › `pub log: String,`

What it printed, for the tail `GateFailure::log_tail` carries.

Kept because the driver's gate failure had `log_tail: String::new()`: a
gate could reject an attempt and the retry that followed was told the
exit code and nothing else. §11.4 sends the diagnostic back to the same
rung, and there was no diagnostic to send.

## `pub struct Verdict` › `pub invocation: InvocationId,`

Which process.

## `pub struct Verdict` › `pub workspace: PathBuf,`

Where it ran — always an exact snapshot, never the task worktree.

## `pub struct Verdict` › `pub code: Option<i32>,`

Its exit code, `None` when it was killed for a timeout or an output
limit.

## `impl Verdict` › `pub fn passed(&self) -> bool {`

Whether the process succeeded.

## `pub struct Judgement {`

What the gate set and the reviewers said.

## `pub struct Judgement` › `pub gates: Vec<Verdict>,`

One per gate, in the order the plan listed them.

A [`Verdict`] and not something richer, because a shell gate's verdict
**is** an exit code — it runs repository-controlled code and reports
whether it passed. Widening this to carry a model and a cost would give
gates fields nothing can fill.

## `pub struct Judgement` › `pub reviews: Vec<ReviewRecord>,`

One per review pass that ran, in the order the plan listed them.

**The record itself, not a verdict.** `AttemptRecord.reviews` is
`Vec<ReviewRecord>` and its emptiness *means* "nothing was reviewed", so
an attempt that was reviewed must produce records or write a false
statement into a log replay can never backfill. A `ReviewRecord`
requires `model` as a `String`, a `cost_usd`, a `preflight_cli_version`
and a typed `ReviewPassOutcome` — none of which an exit code can give,
which is why this is what the review machinery returns rather than what
this module derives from a process result.

## `pub struct Judgement` › `pub failure: Option<AttemptFailure>,`

What the gates and the reviews together say is wrong, if anything.

Decided by the single production authorities — `engine::classify` for a
failed gate, `engine::attempt::review_failure` for a review — because
`ladder::next_step` reads this and `ladder::spends_allowance` derives
the allowance decision from it. A second opinion formed here would
change what a task costs.

## `impl Judgement` › `pub fn timed_out_gate(&self) -> Option<&Verdict> {`

The gate verdict that reached its timeout, if one did. A timed-out gate
is the last verdict — the loop stops at the first refusal — and its
`failure` reads `GateFailed`, which is right for an attempt (PR7's: the
worker gets the log tail as feedback) and wrong at integration, where
`decisions.repairs.not_repairs` lists timeout among the outcomes that
terminate `merge_verification_unavailable` rather than register a repair.
The integration path asks this before it reads `failure`.

## `impl Judgement` › `pub fn accepted(&self) -> bool {`

Whether every gate and every reviewer passed.

**O27's precondition.** `candidate.rs` enters the commit-tree sequence
only for an accepted judgement, and a judgement exists only after every
snapshot in it has been created, executed in, and removed.

One field, not a second walk of the verdicts. This used to re-derive
"did everything pass" by folding the gate and review results, which is a
second opinion about the same question `failure` already answers — and
the two could disagree, because `failure` is decided by
`engine::classify` and `review_failure` while a fold here would be
decided by this line. `ladder::next_step` and
`ladder::spends_allowance` both read `failure`; nothing should read a
different answer to the same question.

## `pub enum AttemptOutcome {`

How an attempt left the run when it did not settle.

## `pub enum AttemptOutcome` › `Interrupted,`

A coordinator died holding it, and the recovering process appended the
terminal.

## `pub enum AttemptOutcome` › `Cancelled,`

The run halted, and the cancellation appended the same terminal.

## `impl AttemptOutcome` › `const fn detail(self) -> &'static str {`

The `detail` the terminal records.

`AttemptInterrupted4` is "never halting … a statement about a
coordinator, not a judgement of the work", so the two outcomes differ in
what they say about *why* and in nothing else.

## `pub struct AttemptContext<'a> {`

---------------------------------------------------------------------------
The context
---------------------------------------------------------------------------

## `pub struct AttemptContext<'a> {`

Everything one attempt needs from the run.

A borrowed bundle rather than eight parameters, because some of them are
process-lifetime ledgers that must be *the run's* and not a fresh one: a
caller that passed a new [`InvocationLedger`] would register against an empty
slot table and never see the hold the table exists to see.

## `pub struct AttemptContext<'a>` › `pub manager: &'a WorkspaceManager,`

The execution root and its funnels.

## `pub struct AttemptContext<'a>` › `pub hooks: &'a mut dyn TopologyHooks,`

The five effect-hook families.

## `pub struct AttemptContext<'a>` › `pub emitter: &'a mut dyn EventEmitter,`

Where a durable event goes. `emit.rs`'s, behind its seam.

## `pub struct AttemptContext<'a>` › `pub runner: &'a dyn Runner,`

The boundary every process of this run crosses.

## `pub struct AttemptContext<'a>` › `pub ledger: &'a mut InvocationLedger,`

R4, with R3's slot table inside it: every Runner process registered exactly
once and settled exactly once, and every agent CLI process's pair held while
it runs.

## `pub struct AttemptContext<'a>` › `pub adapters: &'a dyn AdapterSource,`

Where a reviewer's adapter is resolved from.

A seam and not a plan field: the plan says *which agent* a pass is bound
to, and this says how a name becomes an adapter. A plan carrying the
adapter would need a lifetime, and a plan is a value the driver builds
and holds across the appends it authorizes.

## `pub struct AttemptContext<'a>` › `pub paths: &'a RunPaths,`

The run's directories — where a review's settings and transcripts go.

## `pub struct AttemptContext<'a>` › `pub reviews: &'a dyn ReviewPasses,`

Where a review pass is executed. See [`ReviewPasses`].

## `pub struct AttemptContext<'a>` › `pub input_policy: &'a dyn ReviewInputPolicy,`

Whether the staged evidence is reviewable. See [`ReviewInputPolicy`].

## `impl AttemptContext<'_>` › `fn emit(&mut self, body: TopologyEventBody) -> Result<(), UpstrokeError> {`

Emit, discharging obligation (3) from this context's own ledger.

The seam hands back an [`EmitFailure`] because an ordering module may
not hold the ledger. This one does — it is the same ledger every Runner
process of the attempt registers in — so the obligation is discharged
here and the attempt half keeps returning [`UpstrokeError`].

## `fn emit(&mut self, body: TopologyEventBody) -> Result<(), UpstrokeError> {` › `self.emitter`

Three disjoint field borrows, which is why the hooks are taken from
`self` here rather than passed in: a caller writing
`self.emit(body, self.hooks)` borrows all of `self` twice.

## `impl AttemptContext<'_>` › `pub fn start(`

**O23, and O24's second half.** Append `attempt_started`, then spawn the
worker.

The append is first and the spawn is second, and nothing sits between
them: `T-ATTEMPT`'s boundary is "`attempt_started` (first or retry)
without terminal", and its sub-prefix (a) is "worker running, no
capture". A spawn that preceded the append would put a paid-for process
outside the log entirely, and the recovering process would find a
generation with no attempt and dispatch over the top of it.

### A retry appends through here too

The boundary says "first **or retry**", and one function serves both
because there is one event: a retry's `attempt_started` is one whose
`resume_session` is `Some` and whose [`AttemptPlan::attempt`] is a new
number, and `plan` carries both. What makes it a retry is the decision
taken *before* it — [`super::settle::retry`]'s reservation and
`Worktree.Verify` — and this function does not re-take it. O24's
"converted at `attempt_started(retry)`" is the caller's conversion of
that same reservation at this append.

A `retry` method here would be this function plus a second verification
of a worktree that has already been verified, and its refusal branch
would return while the reservation `settle::retry` took was still held —
never converted, and never cancelled, because cancellation lives in the
failure branch that the first, passing, verify did not take.

**This block used to sit above `fn emit`**, which `bcc5c2f` inserted
between it and this function with no blank line — so all of it attached
to `emit`, `start` rendered undocumented, and `emit`'s own two
paragraphs read as a continuation of the `# Errors` section below.
`PR7-R3-EMIT-004`, the doc-re-targeting class `reviews/FINDINGS.md` §4
records; split 2026-08-26 by moving `emit` above this block rather than
by moving the prose. **Above the `# Errors` heading, not after it**: the
first attempt appended it to the end of the block, and rustdoc renders
everything from a heading to the next heading as that section — so this
paragraph became part of `start`'s error contract, which is the same
rendering failure one level in. `PR7-R5-ATT-006`.

### Errors

Whatever the emitter returns; [`UpstrokeError::Refused`] from the slot
assertion or the invocation ledger; or a runner failure. A non-zero exit
is not an error — it is a [`ProcessOutput`] and the caller's to judge.

## `impl AttemptContext<'_>` › `pub fn run_worker(`

The worker half of [`Self::start`], without the append.

**A retry's `attempt_started` is appended by `settle::retry`**, after
`Worktree.Verify` — the verify is what makes the claim true, and the
fold refuses a second append for the same attempt. So the ready-retry
branch reaches the worker through here and the ready-dispatch branch
through `start`, which is that append plus this.

### Errors

Whatever the Runner returns.

## `impl AttemptContext<'_>` › `pub fn capture(&mut self, site: AttemptSite<'_>) -> Result<Capture, UpstrokeError> {`

**O25.** `Object.CandidateStage` then `Object.CandidateWriteTree`, in the
task worktree.

Two sites and not one, because a kill inside either leaves the same
residue class and a different amount of it: `T-ATTEMPT` sub-prefix (b')
is "git add or write-tree killed after writing objects and before
publishing the index or cache-tree". The staged objects are behind the
**task index** afterwards (R9), which is what makes them recoverable by
scrubbing the worktree rather than by anything cleverer.

**A repair's conflicted paths are read first, and the worker's
resolutions are staged for it.** Two reads name what the manifest governs:
`unresolved_conflicts`, the index's unmerged entries, whatever their
working-tree files look like; and `resolved_conflicts`, the entries a
previous capture of this generation resolved and the index still holds —
Git's resolve-undo record, which staging a resolution over unmerged stages
writes and the funnel's `read-tree --reset` clears before every fresh
materialization. When either names anything, the capture reads the worker's
resolution manifest (`workspace_manager::RESOLUTION_MANIFEST`, a root-level
file the worker writes with its file tools) and reconciles the three
(`plan_resolutions`): every governed path the manifest declares `resolved`
or `deleted`, as the index spells it, is staged by the engine's own
`git add -- :(literal)<path>` or `git rm --quiet --force -- :(literal)<path>`
inside `Object.CandidateStage`, before the `add -A`; an unmerged path it does
not declare, a manifest that does not parse, a path declared both ways, and
a path declared in another case — beside the index's spelling with the other
keyword, or alone — each refuse the whole capture, which returns the base's
tree with those entries and stages nothing, so an unresolved conflict cannot
be captured as the worker's work and a declared subset is never staged
beside a refused one. A resolved path the manifest does not name is left to
the ordinary `add -A`, which stages its edits or its deletion like any
path's — nothing is preserved from the previous capture (PR #249's
fourth-round record review found this note promising that it was); one the
manifest names again is re-staged from the working tree, or removed — the
case `deleted` exists for, a worker whose tools cannot delete correcting
itself in a retained generation (PR #249's third-round regression review
found the manifest unread there and the correction lost). **The manifest
does not outlive the capture that finds it**: `candidate_stage` removes the
worker's file after the `add -A` (a `git clean` of the one untracked path,
by the spelling the checkout lists it under, inside the same funnel) whether
the capture read it or not, so that a declaration is applied once, by the
capture of the attempt that wrote it. The fourth-round regression and
manifest-contract reviews each found a settled `deleted c.txt` reread two
attempts later, once the worker had recreated the file and the ordinary
addition had put an index entry back beside the resolve-undo record, and
the recreated file removed from the disk and the candidate; that round
removed the manifest a capture acted on, and the fifth round's adequacy and
manifest-contract reviews found the same loss one attempt longer — the
deletion re-declared while nothing was governed, the manifest kept unread,
the path recreated, and the declaration read in the attempt after. A
refused manifest is left standing with nothing staged, and so is one whose
capture fails before the removal — the sixth round's manifest-contract
review executed a required clean filter failing the `add -A` after the
declared resolution was staged, and a held `index.lock` before anything
was; a further capture of the worktree would read either again, and the
driver makes none: a refusal fails the attempt as the worker's and is not
resumable, a capture error interrupts it, and either closes the generation,
so the next attempt is a fresh generation and worktree (until PR #249's
sixth repair round this note said a refused manifest stays for the worker
to correct and the next capture reads it). When the index holds nothing the manifest
governs, the manifest is not read at all, and whatever the file says has no
effect; a malformed manifest stages nothing *in a capture that reads it*,
which is the whole of that promise.
After staging, the index is read once more and an unmerged entry left is a
Git error, never a passing capture. A repository that has taken the
manifest's name — a tracked file of it, as spelt or in another case, or a
directory — cannot have a conflict repair declared in it:
`resolution_manifest` refuses before anything is staged, naming what holds
the name, while an ordinary capture there stages that path like any other
(`WorkspaceManager::manifest_name`); a worker's file standing where a
tracked directory of the name was is the worker's, and what the index still
held under the name is staged first, deletions included.
The worker runs no git command (DESIGN §26.4): PR #249's regression review
assembled the production Claude Code and Copilot permissions and found file
tools and gate commands only, so a rule that needed the worker's `git add`
— the first repair round's — could be met by no supported adapter, and the
design-staging decision measured that Codex's `workspace-write` sandbox
cannot admit staging without admitting `commit`. Before that the capture
read the files for `<<<<<<< ` markers; the conformance review materialized
conflicts under `conflict-marker-size=8` and `-merge`, both of which the
scan read as resolved and the capture staged. What a declared resolution's
correctness meets is the configured validation: with no gate and no
reviewer, a file declared resolved with its markers still in it is accepted
(`a_declared_resolution_reaches_the_configured_checks_which_decide_what_they_detect`).

### Errors

The containment refusals or a Git error.

## `impl AttemptContext<'_>` › `pub fn assess(`

The ladder's cheap rungs: **outcome sanity, then the diff.**

**An unresolved conflict pre-empts the diff-shaped verdicts.** A completed
worker that left conflicted paths is reported as that
(`classify::unresolved_conflict_failure`) ahead of `evaluate_outcome`'s
empty-diff verdict — an unresolved capture carries the base's tree, so its
diff is empty for a reason the empty-diff feedback would misdescribe — but
behind the worker's own end: an error exit, a timeout or a question it
asked is reported as what it is.

Both answers come from the production authorities rather than being
formed here — `engine::attempt::evaluate_outcome` for what the worker's
own report says, `engine::classify::diff_failure` for what the diff
alone says. `classify`'s doc already named this caller: "the schema-4
driver reads the same answer rather than forming its own, because
`ladder::next_step` reads the result and the allowance decision is
derived from it".

The third rung is here too, behind [`ReviewInputPolicy`]: staged
evidence whose bytes are not the bytes a gate would see — a clean/smudge
filter, or a dirty submodule behind an unchanged gitlink — cannot be
reviewed completely, and a driver that skipped the check would judge a
transformed blob and call it reviewed.

### Errors

Whatever the adapter's parse returns, or a refusal when the plan's agent
has no adapter.

## `impl AttemptContext<'_>` › `let mut failure = crate::engine::attempt::evaluate_outcome(&outcome, &run.worker);`

In the legacy order, and each rung guarded: a worker that died is not
asked what its diff looked like.

## `impl AttemptContext<'_>` › `if failure.is_none() {`

The third rung: is the staged evidence the evidence a gate would
see. Attributed to the reviewer, not the implementer — a filter or a
dirty submodule is a policy failure of the tree, and the classifier
that decides so is `run_attempt`'s, reused rather than restated.

## `impl AttemptContext<'_>` › `pub fn judge(`

**O26 and O27.** The gate set on one fresh exact snapshot, then each
reviewer on its own.

`decisions.workspace_candidates.snapshots`: "exact snapshot worktrees
(R24) are the only places gates and reviewers execute … one snapshot for
the gate set and one fresh snapshot per reviewer, **never reused across
roles or attempts**, cleaned on completion". The name carries the task,
the generation, the attempt and the role, so "never reused" is a property of
[`SnapshotName`] rather than of this loop's discipline — and, on the
attempt path, each snapshot is removed before the next is created, so a
reviewer cannot inherit the previous one's checkout even by mistake.

### When the snapshots are removed is the caller's, not the judge's

[`SnapshotDisposal`]. The attempt path removes each snapshot as its role
finishes, before `attempt_finished` — the order the effect inventory
registers for `Snapshot.Remove`. The integration path may not:
`pr_sequence[9].slice_contract.side_effect_vs_event_ordering` puts "staging
and snapshot removal (forced) after terminal (incl. Deferred/Parked)", so
its judge leaves every snapshot in place and `integrate` reclaims them once
`merge_prepared`, `merge_rejected` or `merge_verification_unavailable` is
durable. A removal that fails can then no longer strand a completed
judgement behind an unterminated verification, which is what the reviews
of `3414dc58` found (`pr8-triage.md`, F4).

### The name carries the task

Since PR11 phase 3 (the working record's R-Y), [`SnapshotName::gates`] is
`k<key>-g<gen>-a<attempt>-gates` and [`SnapshotName::review`] is
`k<key>-g<gen>-a<attempt>-review<pass>`: the task key is the first parameter of
both constructors, and no constructor was added. Before it, two *different
tasks* at the same generation and attempt named the same slot, which the
sequential substrate never exercised — one attempt ran to completion before
the next began — and the coordinator does at every width above one, where
attempts of different tasks are judged at once. A snapshot name is what
`WorkspaceManager` derives a path from, so two tasks deriving one path would be
a collision no assertion in this module could see; the key makes "never reused"
a property of the name across tasks too. The Windows 220-character budget for a
path ending in `/.git` was re-measured with the longer names (the record's §8).

O26 lives inside [`WorkspaceManager::add_snapshot`], which for a
tree-only input performs commit-tree → intent → add in that order. This
function's contribution to O26 is passing [`SnapshotInput::Tree`] — an
integration snapshot checks out an existing commit and creates no object,
and passing the wrong one here would silently skip the whole clause.

### Errors

The containment refusals, a Git error, a slot or ledger refusal, or a
runner failure.

## `impl AttemptContext<'_>` › `let mut failure = assessed.failure.clone();`

**The cheap rungs carry forward, and they short-circuit the expensive
ones.** `run_attempt` guards every rung with `failure.is_none()`, so
a worker that died or produced no diff is not asked to pass gates and
is certainly not shown to a frontier reviewer. Starting this at
`None` would run the whole verification ladder over an empty diff.

## `impl AttemptContext<'_>` › `log_tail: crate::util::tail(`

§11.1's tail, which is what §11.4 sends back to the
same rung. This was empty.

## `impl AttemptContext<'_>` › `if refused {`

**First failure ends the set**, which is `gates::run_all`'s own
shape: it `return`s the first `GateFailure` rather than running
the rest. Two consequences, and both matter. A refused gate
must not buy the gates after it — the diff is already
rejected. And the first cause must survive: a later gate that
fails to *spawn* would otherwise overwrite the real failure
with an infrastructure one, and `ladder::next_step` would
price the attempt from the wrong kind.

## `impl AttemptContext<'_>` › `if failure.is_some() {`

A failed gate is a rejection of the work, and a reviewer judging
a diff the gates already refused is a frontier invocation bought
to learn nothing. `run_attempt` short-circuits for the same
reason and §11.1 is the same sentence for gates.

## `impl AttemptContext<'_>` › `&invocations(pass),`

**Caller-supplied, ordinal and all.** Nothing pass-shaped is
minted here: PR8's merge verification is this machinery's
third caller and its identities come from
`SequenceIdentities`, not `AttemptIdentities`. A `judge` that
reached for either scheme would be a seam its next caller had
to redesign.

## `impl AttemptContext<'_>` › `let ids = invocations(pass);`

**R4, for the processes the seam ran.** `permits.protocol` is
"every Runner process is registered and settled exactly once",
and a review pass reaches the Runner through `run_review` with
the raw handle — so the worker and the gates were in the ledger
and the reviewers and their re-asks were not.

Reconciled after the call rather than before it, because the seam
owns the spawning and only its outcome knows how many processes
ran: `ReviewOutcome::invocations` counts the pass plus each
re-ask. Registering a re-ask up front would put an id in the
ledger for a process that may never exist, which is the opposite
failure.

The ids are the caller's own — the same `ReviewInvocations` the
pass was given — so this records what ran under the names it ran
under rather than minting new ones.

## `impl AttemptContext<'_>` › `let unavailable = matches!(outcome.result, review::ReviewResult::Unavailable { .. });`

Read before the result is consumed: a judge that never ran is not
a judge that said no, and the ledger has to show which happened.

## `impl AttemptContext<'_>` › `fn snapshot(`

One exact snapshot of the captured tree, on the recorded parent.

## `impl AttemptContext<'_>` › `pub fn settle_interrupted(`

`T-ATTEMPT`'s resume action, for a coordinator that died or a run that
halted.

> "append `attempt_interrupted` (unknown spend, allowance refunded,
> generation Closed, lease by kind); discard residue: snapshot intents
> and worktrees reclaimed (releasing an ephemeral commit to R27), the
> task worktree scrubbed with force (releasing staged objects to R27 and
> removing any index.lock or other administrative residue); an ephemeral
> commit without a snapshot and objects written by an interrupted capture
> are left to Git (nothing to reclaim)"

The event is first and the reclaim second, for the same reason a run-end
closure is: until the terminal is durable the log still says this attempt
is in flight, and a scrub before it would remove the worktree the next
resume would look for.

"Nothing to reclaim" is load-bearing and is why this function does not
go looking for unreferenced objects. An ephemeral commit written before
its intent, and blobs written by a killed `git add`, are R27 — **Git's**
— and an engine that pruned them would be establishing authority over
the object store, which `cleanup` forbids ("cleanup is expected-path,
contained, idempotent, and never establishes authority").

### Scope of the snapshot reclaim

This attempt's own snapshots, and nothing else in the snapshot namespace: the
gate snapshot and every review snapshot its [`JudgeNames`] produce
([`JudgeNames::owns`]). At `max_parallel = 1` that was every snapshot intent
of the execution root, and the reclaim used to remove them all; since PR11 phase
3 another task's attempt may be judging beside this one, and a reclaim that
removed a sibling's snapshot would take its gates down with it. A later attempt
of this task and an integration's snapshots are not this attempt's either.

### Errors

Whatever the emitter returns, or a Git or I/O error from the reclaim.

## `impl AttemptContext<'_>` › `pub fn cancel_in_flight(`

The source retains the cancellation ordering protocol required by §10.

The cancellation half of `T-ATTEMPT`: "at Halted the same terminal is
appended by cancellation".

`permits.protocol`: "cancel(invocation_id): a pending slotted request is
removed …; a granted or non-slotted running invocation is cancelled
**after the Runner terminated its process or container**". In the
sequential substrate `Runner::run` is synchronous, so a halt observed by
this coordinator is observed between invocations and there is no live
child to terminate — but the *ledger* may still carry a registration
whose completion never ran, and leaving it registered would make the
process-end balance check pass over a leak. So the ledger is cancelled
first, the slot released with it, and the terminal appended after.

### Errors

As [`Self::settle_interrupted`].

## `impl AttemptContext<'_>` › `let cancelled = self.ledger.cancel_all_running();`

Every in-flight registration cancelled, and with it every pair held or
waited for, without naming one, because what is being cancelled is whatever
this process still holds. Naming one would be asserting *which*, and a halt
is the moment that assertion is least safe: the pair may belong to the
worker, to a reviewer, or to a re-ask, and a guess that missed would leave
the ledger unbalanced at process end with nothing to say so. The pair goes
with the registration because the table is inside the ledger, so the two
cannot disagree.

## `impl AttemptContext<'_>` › `fn discard_residue(`

This attempt's snapshots reclaimed, then the task worktree scrubbed with force.
The attempt number is the caller's, so the names are the ones this attempt's
judge would have produced.

## `impl AttemptContext<'_>` › `fn execute(`

The source retains the registration, slot and settlement protocol required by §10.

Admit the invocation through the judge's registrar — with its slot pair if the
identity is slotted — run it, and report how it ended: completed or cancelled,
which releases the pair, or failed with its process unresolved, which keeps
the registration running and its pair held until this process exits (round R1
of the PR11 record).

`permits.protocol` in order: "register(invocation_id, slots) -> if
slotted, wait for the atomic pair grant … -> Runner spawn ->
complete(invocation_id) releasing any slots". Who waits depends on the
registrar ([`crate::engine::topology::preflight::Registrar`]). The
synchronous one, which `step` and every width-1 caller use, is the ledger
itself, and nothing waits: the coordinator runs the process on its own thread,
INV-18's coordinator "never blocks on an entitlement, provisional reservation,
or slot", so a pair not grantable at once is a leaked hold rather than
contention, and [`InvocationLedger::register_at_once`] withdraws the request and
refuses it (R-O). A spawned pipeline's registrar is the coordinator's channel
client: it waits — on its own thread — for the grant the coordinator loop sends
when both slots are free (`coordinator.md`).

The call carries the judge's [`Carried`]: at width 1 a fresh cancellation that
never fires and no lease paths, so the call is the one `run_blocking` made; in
a pipeline the pipeline's cancellation, which the coordinator fires on a halt,
a shutdown or an error, and the run's cleanup-lease paths (R-Z).

### The registration's end is reported on every path out

`admit` either leaves a registration running (its pair granted) or leaves none
unsettled: a refused request was withdrawn inside the ledger, registered and
cancelled once. From there the only step between
the registration and the report of its end is the Runner call, and both of its
answers are reported. A success settles it completed, and a failure whose
process the Runner established as gone settles it cancelled. A failure whose
process the Runner could not establish as ended (`ProcessFate::Unresolved`)
settles nothing: `InvocationLedger::end` keeps the registration running and
its pair held until this process exits, because `permits.protocol` releases a
granted invocation only after its termination, and the ledger does not balance
while it is held. `permits.protocol` settles an invocation "exactly once", and
[`InvocationLedger::balances`] states that as "no entry is `Pending` or
`Running`" — so a `?` between the register and the end report would abandon
an entry that at process end is **indistinguishable** from a process this
coordinator genuinely lost. A pair the substrate cannot be granted is not a
lost process; it is a process that never started, and reporting it as a leak
would spend a real signal on a bookkeeping mistake.

## `impl Judge<'_> {` › `.ended(&request.invocation, InvocationEnd::of(&outcome))`

`permits.protocol` settles an invocation exactly once, and the
settlements are not interchangeable: a process the Runner could not
start or supervise never completed, and recording it as completed would
put a failure in the ledger under the name of a success; and a process
whose end the Runner could not establish is not settled at all, but
kept with its pair (`InvocationEnd`, round R1 of the PR11 record). The
registrar is told how the call ended, not merely whether it succeeded.

## `impl AttemptContext<'_>` › `fn pair_for(`

What the registration asks for. An unslotted invocation — a gate — asks for no
pair; a slotted one asks for `{agent, pool?}`, with the agent from the request
and the pool from the plan, under the judge's standing.

A slotted request naming no agent is refused before anything is
registered: the pair it would take has no agent to key it by. Every error
here therefore leaves the ledger settled, which is what lets
[`Self::execute_typed`] use `?` on it.

## `impl AttemptContext<'_>` › `fn verdict(`

[`Self::execute`], reduced to what a judgement records — through
[`Judge::execute_typed`], so the Runner's own error stays
[`JudgeError::Runner`] all the way up.

## `pub enum JudgeError {`

Why a judgement could not be completed.

A Runner that could not run a gate process is told apart from every other
error, because the two have different terminals at integration:
`invariants[INV-23]` classifies a mid-run spawn failure as a
`RunnerSpawnFailure` outage settlement, and
`transaction_fault_matrix[T-VERIFY].resume_action` requires an observed
infrastructure failure to terminate `merge_verification_unavailable`
deferred or parked — never to escape as an error that leaves the
verification open and the defer count untouched. A reviewer's process
failure already reaches the judgement as `ReviewResult::Unavailable`
through `review::run_review`; this covers the gate path through
[`Judge::execute`]. The attempt path converts it back into the plain
error it always was ([`From`]), so nothing there changes.

## `impl AttemptContext<'_>` › `log: format!("{}{}", output.stdout, output.stderr),`

Both streams, in the order `gates::run_all` joins them: a gate's
diagnostic is as often on stderr as on stdout.

## `pub struct VerificationRequest<'a> {`

What an integration verification is planned from: the task whose
candidate is being integrated, and the implementer binding the candidate
ran under, so the review passes are the ones its own review would run.

## `pub struct VerificationPlan {`

Every recorded gate and every review pass an integration reruns on the
proposed tree: `DESIGN.md` §26.3, "rerun every recorded gate and review on
the proposed integrated tree".

## `impl AttemptContext<'_>` › `let ledger = std::sync::Mutex::new(&mut *self.ledger);`

The judge [`Self::run_worker`] and [`Self::judge`] build: over this context's
ledger as the synchronous registrar, reborrowed for one worker or one judgement,
with a [`Carried`] that cancels nothing, and carrying the standing of the attempt
the run's identities name.

The standing is read from the fold through the emitter
([`EventEmitter::standing`]), because the fold is the emitter's to hold and
this module reads nothing of it but this: whether the attempt is the one the
fold has in flight, which is what admits its slotted requests
(`permits.deadlock_freedom`'s acquisition order).

## `pub enum SnapshotOf {`

What a judgement runs against: the exact tree a candidate captured, or the
commit an integration proposes.

`decisions.workspace_candidates.snapshots`: a tree-only input is committed
ephemerally on its recorded parent before the snapshot is added; a commit
input is checked out as it is and creates no object.

## `pub enum JudgeNames {`

How the snapshots of one judgement are named: one for the gate set, one
fresh per reviewer, never reused across roles.

## `pub enum JudgeIdentities {`

Whose invocations a judgement's processes are: an attempt's
`(key, generation, attempt, role, ordinal)` or an integration's
`(sequence, role, ordinal)`.

## `pub enum SnapshotDisposal {`

When a judgement's snapshots are removed.

The attempt path removes each snapshot as its role finishes, before
`attempt_finished` — the order the effect inventory registers for
`Snapshot.Remove` (`Before(AttemptFinished)`). The integration path may
not: `pr_sequence[9].slice_contract.side_effect_vs_event_ordering` puts
"staging and snapshot removal (forced) after terminal (incl.
Deferred/Parked)", so its judge leaves every snapshot in place and the
sequence reclaims them once the terminal is durable — a removal that
failed can then no longer strand a completed judgement behind an
unterminated verification.

## `pub enum SnapshotDisposal {` › `AsEachRoleFinishes,`

Remove each snapshot as soon as its gate set or reviewer is done.

## `pub enum SnapshotDisposal {` › `AfterTheTerminal,`

Leave every snapshot, with its intent, for the caller to reclaim
after the terminal it appends.

## `pub struct Subject<'s> {`

One thing to judge: what to snapshot, what to run in the snapshots, and
what the reviewers are told.

## `pub struct Subject<'s> {` › `pub disposal: SnapshotDisposal,`

When the snapshots this judgement creates are removed.

## `pub struct Subject<'s> {` › `pub stem: String,`

The stem the review transcripts are filed under.

## `pub struct Subject<'s> {` › `pub prior_failure: Option<AttemptFailure>,`

A failure already decided before anything ran — an assessment's — so
the gate set and the reviewers are skipped exactly as the attempt path
skips them.

## `pub struct Subject<'s> {` › `pub invocations: &'s dyn Fn(u32) -> review::ReviewInvocations,`

The pass and re-ask invocation ids of review pass `n`.

## `pub enum JudgeError {` › `Runner(RunnerError),`

The Runner returned an error instead of a process output, with the
[`crate::error::ProcessFate`] it established for the invocation's process.
The integration verification decides by that fate — `NeverStarted` and
`Gone` are observed outages with a terminal of their own, `Unresolved` ends
the command — because the repair round of `3414dc58` had settled every
Runner error as a spawn failure and the reviews of `916852c9` reproduced a
gate still running in Docker beside a `Deferred` terminal that had already
released the transaction and removed its snapshot.

## `pub enum JudgeError {` › `Other(UpstrokeError),`

Anything else: a snapshot funnel refusal, a ledger refusal, an adapter
no pass answers to, or a review pass that could not be run.

## `pub struct Judge<'a> {`

The gate set and the reviewers, run on fresh exact snapshots.

The one implementation of "gates on a fresh exact snapshot, each reviewer
on its own fresh exact snapshot" for both the candidate phase and the
integration: a candidate is judged on the tree it captured, an integration
on the proposal or head commit, and everything else — the snapshot per
role, the invocation ledger, the slot pair, the review records — is the
same protocol run once.

## `pub struct Judge<'a>` › `pub standing: Standing,`

The standing of the pipeline this judgement runs for — an attempt in flight,
or a verification started — read from the fold by whoever built the judge
and presented with every slotted registration it makes. The synchronous
registrar checks it: at width 1 nothing is appended while a judgement runs, so
the fold it was read from cannot move under it. A spawned pipeline's registrar
does not use it: the coordinator appends while pipelines run, so it reads the
pipeline's standing again, from the fold, when it processes each registration
(the working record's R-N and R-U); the value here is the spawn-time read.

## `pub struct Judge<'a>` › `pub registrar: &'a dyn Registrar,`

Where every invocation of this judgement is admitted and ended: the ledger
behind a `Mutex` for the synchronous substrate, the coordinator's channel
client for a spawned pipeline. The judge's own calls and the registering
boundary around each review pass go through the same one, so there is one
registration path whichever thread runs the judgement.

## `pub struct Judge<'a>` › `pub carried: &'a Carried,`

What every Runner call of this judgement carries: the pipeline's cancellation
and the run's cleanup-lease paths. The judge builds its own calls from it and
the registering boundary rebuilds a review pass's calls with it.

## `impl Judge<'_> {` › `.carrying(self.carried);`

Each review pass runs through a registering boundary over this judge's
registrar, so each of its processes — the pass, and a re-ask — is registered
with its pair before it starts and settled after it ends, and each carries the
judge's cancellation and lease paths. This closes
`PR7-R3-ATTEMPT-002-REVIEWERS-TAKE-NO-SLOT`: the pass used to be handed the
raw runner and entered in the ledger only after it returned, holding no
pair while it ran. In a spawned pipeline the registration is the
coordinator's, made when it processes the pass's `Admit`.

The registrar is behind `&dyn Registrar`, which is `Sync`: the boundary is a
`Runner`, `Runner::run` takes `&self`, and a `Runner` is `Send + Sync`. The
synchronous registrar's lock guards the ledger's register and settle calls
only, never the Runner call between them. `src/review.rs` tests
`RunnerError::is_cancelled` first since PR11 phase 3 (a cancelled pass is an
error, never an unavailable reviewer); the legacy engine never cancels, so its
review path is otherwise unchanged.

## `impl Judge<'_> {` › `if invocations != through_the_pair {`

The pass reports how many of its processes returned, and the boundary counts
how many it completed. They differ only when the pass ran a process
somewhere other than the runner it was handed — outside the broker, holding
no pair — and the judgement is refused. It is the same kind of check P4 makes
of a probe (`create.rs`, `used_the_grant`): handing a pass a capability
cannot make it use one, so the use is measured.

It sits after the charge, so a pass that returned is charged whatever this
check finds.

## `impl Judge<'_> {` › `workspace: snapshot.path(),`

Each reviewer runs in the fresh snapshot taken for its pass and nowhere
else (`verification_isolation`: "one for the gate set and one fresh per
reviewer"). This line is held by tests that observe the reviewer's actual
checkout — `integrate::tests` through the scaffold's spawning review
double, `recover::tests` through the driven loop's — after the cover
review of `8a5f59e8` pointed it into the staging worktree for integration
reviewers and passed all 366 engine-topology tests (`PR8-R4-REVIEW-ORACLE`).

## `impl Judge<'_> {` › `pub fn judge(&mut self, subject: &Subject<'_>) -> Result<Judgement, JudgeError> {`


Run every gate on one snapshot, then every reviewer on its own, and
say what they decided.

### Errors

[`JudgeError::Runner`] when the Runner could not run a gate process;
[`JudgeError::Other`] for a snapshot funnel refusal, an adapter no pass
answers to, or a review pass that could not be run.

## `impl Judge<'_> {` › `if let super::dispatch::Whole::No(found) =`

**Placement 2** (the external-prune follow-up, F, under O1 option A; #329's
record §8.4): every gate and review verdict — a failure, and a pass — is
re-checked against its own snapshot's worktree registration before the
snapshot is released, and a verdict read while it is not whole is a registry
refusal (`JudgeError::Other(RegistryRefused)`), never judged. The gates' check
runs once, after the gate loop and before the gates' snapshot is released; a
review pass's runs after its charge (`account.charge`) and after the
process-count check, and before its snapshot is released.

- **Every verdict.** A gate is the user's process: one that reads its
  snapshot's index answers through a deleted `index` as through an empty one
  and can pass where it would fail. So a pass is checked as a failure is. The
  cost, pinned: a genuine pass in a snapshot whose registration a prune
  deleted while the role ran is refused too; a verification verifies again in
  fresh snapshots at the next resume, and an attempt is settled interrupted
  and runs again in a new generation, its spend possibly repeating.
- **After the charge.** Under O14(b) the refused pass is charged and recorded,
  as the process-count refusal's pass already is; under O14(a) the order
  changes nothing.
- **After the process-count check.** That refusal is master's: when both
  apply it stays the error returned, and the snapshot is left as it leaves it.
- **Before the release.** For an attempt the snapshot is released as each role
  finishes, and the release removes the snapshot and its registration, after
  which a whole registration and a deleted one look alike. A verification's
  snapshots are kept until after the terminal, so there the order is moot.

A refusal leaves its snapshot unreleased, as the process-count refusal does;
the next resume reclaims it with the rest of the interrupted command.

## `impl Judge<'_> {` › `fn execute_typed(`

[`Self::execute`], telling a Runner error apart from a registration or slot
refusal: the Runner's own `Err` is [`JudgeError::Runner`]. Either way the call
is ended through the registrar with how it ended (`InvocationEnd::of`), so the
fate the Runner established reaches the ledger: an error whose fate says the
process is gone cancels the registration, and one whose fate is unresolved keeps
it and its pair, since nothing established that the process ended (round R1 of
the PR11 record, the early review's `R1-CONC-1`; on a pipeline, the coordinator
also stops everything).

## `pub trait ReviewAccount {`

Where a completed review pass's cost is charged.

A review that returned has been paid for, whatever becomes of the judgement it
belongs to. [`Judge::judge`] can still fail after one — the next reviewer's
snapshot, the invocation ledger, an absent adapter — and those exits carry no
[`Judgement`] and so no reviews, while an integration that settles on a Git
error settles *unavailable* rather than ending the command, and the loop then
admits another sequence in the same incarnation against a run total the reviews
are missing from. So the charge is reported here as each pass completes rather
than read off the judgement that returns, and every exit from `judge` leaves
the account holding what was spent.

A pass whose `run` returns an error is not charged: no outcome means no cost was
reported, and unknown spend is reported as unknown (INV-14).

## `pub trait ReviewAccount {` › `fn charge(&mut self, review: &ReviewRecord);`

Charge one completed review pass, whose reported cost may be unknown.

The whole record and not just its cost, because the caller that charges is
also the one that has to *record* what it charged. An integration verification
ends in a durable terminal carrying its review records, and on the arm where
`judge` failed after a paid pass there is no `Judgement` to take them from — so
the account is the only thing that saw them. Handing over the record costs the
callers that keep no account nothing: [`NoReviewAccount`] ignores it.

## `pub struct NoReviewAccount;`

The account of a caller that keeps none.

The legacy attempt path charges from the durable `AttemptRecord` its settlement
writes, and the test scaffold judges nothing it pays for. It is a named type
rather than an `Option`, so a caller that judges cannot reach `judge` without
saying what it does with the cost.

## `for (index, reviewer) in subject.reviewers.iter().enumerate() {` › `account.charge(&record);`

The pass has returned, so its cost is spent. Charge it before anything below
can fail and discard the judgement: the grant check, the snapshot removal,
and the next iteration's snapshot creation all return early from here on.

The record is built first and charged from, then pushed, so the account and
`reviews` hold the same value and the charge still sits above every `?` that
follows. Nothing fallible runs between the pass returning and the charge.

## `impl JudgeNames` › `pub fn owns(self, name: &SnapshotName) -> bool {`

Whether `name` is one of the snapshots these names produce: the gate snapshot,
or a review snapshot of any pass. The review stem is derived from the
constructor itself (`review(0)` without its trailing `0`), so the format is
written once, in `naming.rs`, and a name only matches when what follows the
stem is a pass number. It scopes an interrupted attempt's reclaim
([`AttemptContext::settle_interrupted`]).

## `impl JudgeNames` › `const fn gated(self) -> bool {`

Only an attempt's snapshots go through the coordinator's snapshot gate
(`coordinator.md`, R-W): the frozen stale integration reclaims every snapshot
of the execution root, so an attempt's may not be live while it does; an
integration's own snapshots are the ones it reclaims.

## `impl Judge<'_> {` › `fn snapshot(`

One exact snapshot of the subject, taken after the registrar grants it
(`snapshot_begin`): the synchronous registrar grants at once, a pipeline's
asks the coordinator's snapshot gate and waits.

## `impl Judge<'_> {` › `fn release(`

The snapshot removed, then reported to the registrar (`snapshot_end`), so the
gate counts it gone only once it is.

## `impl AttemptContext<'_>` › `pub fn announce(`

`attempt_started` alone, for the callers that append it and run the attempt
apart: the width-1 loop and the coordinator both announce on the coordinator
thread and then run [`attempt_body`] (inline or spawned). [`Self::start`] is
announce then [`Self::run_worker`], for the callers that do both at once.

## `impl Judge<'_> {` › `pub fn run_worker(`

The worker's request, built from the plan and the site, executed through the
registrar. Shared by [`AttemptContext::run_worker`] and [`attempt_body`].

## `impl Judge<'_> {` › `pub fn judge_attempt(`

An attempt's judgement: the captured tree on its recorded parent, snapshots as
each role finishes, the attempt's names and identities. Shared by
[`AttemptContext::judge`] and [`attempt_body`].

## `impl Judge<'_> {` › `if assessed.failure.is_some() {`

**Placement 3** (the external-prune follow-up, F): an attempt's assessed
failure — the worker's own end, an unresolved capture, a diff-shaped verdict,
or the review-input policy's answer — is re-checked against the attempt's own
slot before it is judged and settled, so that `settle_judged` never spends an
attempt on a failure read while the slot's worktree registration is not whole.
When the failure is the policy's own answer, C1-AP checked just before, and
placement 3 meets only a deletion after that check; when the assessment
bypassed the policy (it reads the policy only while no failure is assessed),
placement 3 is the first check after the capture's last one, and it meets
every deletion that lands after the capture's last read of the index, an
`index`-only one included: nothing in that interval reads or writes the slot's
index.

## `fn capture_tree(`

The capture, as a function of the manager, the hooks and the site, so the
pipeline body runs it without an emitter. The comment inside is the capture's
own contract, unchanged.

## `struct Assessor<'a> {`

The assessment's two seams, so [`AttemptContext::assess`] and [`attempt_body`]
run one implementation.

## `struct Assessor<'a> {` › `manager: &'a WorkspaceManager,`

The manager whose common git dir C1-AP's check reads the attempt's slot
against (`super::dispatch::read_in_whole_checkout`).

## `impl Assessor<'_>` › `if let Some(problem) = super::dispatch::read_in_whole_checkout(`

**C1-AP** (the external-prune follow-up, F): the attempt's own review-input
read is used only while the attempt's slot registration is whole. With the
slot's `index` deleted the legacy policy reads an empty index and answers no
problem where a genuine unstaged change is one (the design's E8), so the
attempt would be judged on gates and reviews where a `review_input_failure`
was due; the read is a registry refusal instead, and the command ends
resumably. Its Git error ends the command whatever its variant, as it did.

## `pub struct AttemptJob {`

Everything an attempt's body needs that the coordinator knows when it starts
the attempt: the site (key, generation, base, slot, worktree), the plan and the
frozen registry entry, owned, so it can move to a pipeline's thread. The
coordinator keeps its own copy for the settlement.

## `pub struct Judged {`

What an attempt's body returns and a settlement consumes: the capture, the
assessment and the judgement. It is the payload of the coordinator's `Judged`
hand-back, bound there to the attempt's identity.

## `pub struct Work<'a> {`

Everything an attempt's or a verification's body touches — and nothing it must
not: no emitter and no fold, because a body runs between two appends of one
identity and appends nothing (the working record's R-S). The width-1 loop
builds it from its own seams with the synchronous registrar; the coordinator's
pipelines build it from owned seams with the channel client.

## `pub fn attempt_body(`

The attempt between `attempt_started` and its settlement: the worker, the
capture, the candidate diff, the assessment, the review inputs and the
judgement, in the order `step` ran them before PR11 phase 3. `step` runs it
inline on its own thread with its own hooks object, so every kill point and
hook test keeps its meaning; the coordinator runs the same function on a
pipeline thread.
