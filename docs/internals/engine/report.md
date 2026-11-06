# `src/engine/report.rs`

Extended notes for [`src/engine/report.rs`](../../../src/engine/report.rs).

These notes preserve the module comments after the status repairs. Item headings quote source lines for navigation.

## Module

The remaining write! assembles a ledger cell into String, whose fmt::Write
implementation cannot return Err. The discard does not hide an I/O error.

## `#![forbid(`

**Fenced against an allow above it, and behaviour-preserving.** From #306
until 2026-09-20 `src/engine/mod.rs` carried
`#![allow(clippy::disallowed_methods)]` for the two conductor entry points its
facade called, and a lint level inherits down the module tree: this file wrote
no attribute, so it inherited that allow, and the placement scan -- which
records what a file writes -- recorded nothing.
The third review of #306 (`PR306-FACADE-ALLOW-ESCAPES-TO-SIBLINGS`) proved
the consequence in `assembly.rs`, a sibling under the same parent: a `pub(super) fn` calling `std::fs::write`,
referenced from a production body under `engine::topology`, passed clippy and
the whole suite. This attribute restores the level the file had before the
facade's allow existed -- the three governed lints were already errors under
`-D warnings` -- and changes nothing else: no statement here reaches a denied
primitive, so the deny reddens nothing today and only matters against an
inherited allow. It is the form `src/engine/topology.rs` wrote first, needs no
row in `effects/allowlist.toml` (the scan records `allow` and `expect`, never
`deny`), and
`effects::tests::every_child_the_engine_facade_declares_re_denies_or_records_what_it_inherits`
refuses its absence.

**The facade's allow is gone, and the fence stays.** On 2026-09-20 the
facade's entry points moved into `coordinator` and `resume`, the facade stopped
calling anything denied, and its allow was removed with its row
(`PR306-FACADE-INLINE-ESCAPE`); it denies the same three lints itself now. So
nothing above this file allows anything today. The guard holds this attribute
regardless, for every module the facade declares: a module that leans on its
parent's level is exempt the day that level changes, which is exactly what #306
did to this one.

## `Parked {`

Waiting on a human. The rest of the run kept moving (invariant 6), and
nothing about this task is lost — the question carries its context.

## `Blocked {`

A dependency failed, or parked and was never answered.

## `Skipped,`

Not attempted because the run halted earlier.

## `Running {`

An attempt is running right now. Only a live `status` produces this: a
run that has ended has nothing left in flight.

## `Queued,`

Its turn has not come yet, and the run is still going — distinct from
`Skipped`, which means the run ended before this task got a turn.

## `#[serde(other)]`

A status this build does not know, from a `report.json` a newer upstroke
wrote. Never produced by this crate.

`report.json` is a projection for whoever reads the run afterwards, and
this enum is `pub` and `Deserialize` because that reader may be someone
else's program. Without a fallback, every variant added here is a hard
`unknown variant` error in every consumer built against an older
version — which is what `running`, `Queued` and this one did to anything
compiled against 0.0.1, and that break is already published. Adding it
now cannot undo that; it stops the next one.

## `pub model: String,`

The final attempt's implementer model. `cost_usd` is the implementer's
spend across every attempt; reviewer spend is a separate field because
it is a different model at a different tier, and folding them together
makes cheap rungs look expensive to anyone reading the ledger (§13).

## `pub review_models: Vec<String>,`

Every model that judged this task, in the order first seen.

Across *all* attempts, not just the last, because `review_cost_usd`
beside it sums all of them — a list scoped to the final attempt next to
a total scoped to every attempt reads as though one explains the other.

## `pub review_cost_incomplete: bool,`

At least one review pass reported no spend, so `review_cost_usd` is a
floor (§13). Rendered as a `?` rather than left to look exact.

## `pub attempts: Vec<AttemptRecord>,`

Every attempt, oldest first — the escalation trail.

## `pub fn total_cost_usd(&self) -> Option<f64> {`

Implementer plus reviewer, across every attempt.

## `pub fn cost_incomplete(&self) -> bool {`

Whether an attempt reported no spend, making `cost_usd` a floor.

The worker-side twin of [`Self::review_cost_incomplete`], and a method
rather than a field because it is derivable from the attempts already
carried here — no schema change, so an older `report.json` reads back
with the same answer this computes.

Two kinds of attempt land here, and both genuinely spent something
nobody can name: one on a route that reports no dollars at all (Codex
reports tokens — §13), and one the engine was killed inside, whose
`cost_usd` is `null` precisely because the record of its ending was
never written. `unpriced_attempts` counts the same condition for the
capacity estimator, so the ledger and the estimator now agree about
which attempts are unpriced.

## `pub fn trail(&self) -> String {`

Compact escalation trail, e.g. `small×2 failed → mid ok`.

## `#[derive(Debug, Clone, Copy, PartialEq, Eq)]`

How the run ended.

`Parked` is deliberately not `Halted`: §12 requires CI to tell a clean
completion from one that left questions unanswered. `BudgetExceeded` earns
its own variant for the same reason one step further out — "your ceiling
stopped it" is neither a failure nor a question, and `upstroke resume` means
something different after each of the three.

## `pub gates: Vec<String>,`

Effective gate names, and whether they came from config or derivation.

## `pub halted_at: Option<String>,`

Task id the run halted at, if any.

## `pub questions: Vec<QuestionRecord>,`

Every question raised, with its answer where one arrived (§12).

## `#[serde(default)]`

The §13 ceiling that stopped the run, if one did.

## `#[serde(default)]`

What each pool drained (§13), folded from this run's own attempts: each
attempt record, and each review pass it carries, in the pool it names
(`capacity::drain_of`). A schema-4 report also counts each integration review
pass that names a pool, once (`from_topology`, below).

## `#[serde(default)]`

Whether an engine is driving this run right now. A live run must not be
rendered as a finished one: its in-flight attempt has not failed, and
the tasks queued behind it have not been skipped.

## `#[serde(default)]`

Whether this run stopped without ever recording that it finished — the
signature of a kill, a power loss, or an aborting error.

A run in that state has no outcome, and `outcome()` cannot tell: a
killed run has nothing halted, no budget stop and nothing parked, which
is indistinguishable from a clean finish. So the flag has to be carried
rather than derived, exactly as `running` is.

Not to be confused with `RunStatus::interrupted`, which is a `u32`
counting the attempts that were cut off mid-flight. This is the yes/no.

## `#[serde(default, skip_serializing_if = "std::ops::Not::not")]`

`questions_in_log`: whether this run keeps its questions in its log rather
than in payload files. A schema-1..3 run writes one file per question under
`.upstroke/runs/<id>/questions`, and `render` points at that directory. A
schema-4 run writes none, so its summary says to use `upstroke answer <question
id>` instead (`from_topology` sets it). Until brief 3's S9 witness, `upstroke
run` and `resume` on a schema-4 run named a directory that is not there
(V129, case E).

False, the legacy value, is the default and is never serialized, so a
legacy report's JSON is byte-for-byte what it was.

## `pub unattributed_cost_incomplete: bool,`

Whether review spend the log attributes to no task is unknown: an
integration review pass that no verification start pairs with reported no
cost, or a `merge_verification_interrupted` that no start pairs with (K4,
`select::IntegrationReviews::unattributed_spend_unknown`). No task row can
carry it, so it is carried here and [`RunReport::total_is_floor`] reads it.
A valid log has neither: the fold admits a verification's terminal only
after its start. Only `from_topology` sets it; `build_report` leaves it
false. Defaulted and never serialized when false, as `questions_in_log` is,
so a legacy report's JSON is unchanged.

## `#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]`

One pool's line in the ledger: what this run drew from which subscription.

## `pub cost_usd: Option<f64>,`

Reported api-equivalent dollars, `None` when nothing on this pool
reported any.

## `pub unpriced: u32,`

Attempts whose route reports no spend at all (§13), making the figure
above a floor rather than a total.

## `pub fn total_is_floor(&self) -> bool {`

Whether `total_cost_usd` is a floor rather than a figure.

`total_cost_usd` is an `f64`, so it cannot say this for itself: a run
that reported nothing and a run that genuinely cost nothing both arrive
as `0.0`. The distinction has to be carried alongside, and §13 is
explicit that a ledger which cannot tell free from unreported is worse
than no ledger.

Both halves count. The review side has been marked since step 9; the
worker side became reachable the moment an implementer could report
tokens without dollars, and is now the *normal* case for a
codex-implemented run rather than an edge one. A schema-4 report also
counts the review spend no task row carries (`unattributed_cost_incomplete`,
K4).

For a schema-4 run this is the one treatment of unknown spend: the spend
approval asks it of the report of the same log (`engine::topology::run`'s
`spend_is_a_floor`, K5), so what the ledger marks `?` and what the approval calls
a floor cannot differ.

## `fn committed_count(&self) -> usize {`

How much of the plan actually landed — the one figure every ending
wants, whether the run finished, is still going, or was cut off.

## `pub fn outcome(&self) -> RunOutcome {`

Precedence: a halt outranks a budget stop, which outranks parked work.

That order falls out of what actually happened rather than being a
policy: a halt stops the drain before any further budget check can run,
so a run with both is one that halted and then found its ceiling
irrelevant. And a budget stop leaves tasks parked-or-skipped behind it,
so reporting `Parked` would name a symptom instead of the cause.

## `pub fn from_state(`

Build a report from a replayed log.

`status` and the `report.json` a run writes go through the same
function, so what an operator sees mid-run and what the file says
afterwards cannot drift into disagreeing.

## `pub(crate) fn from_topology(`

The schema-4 run's [`RunReport`], projected from the finished fold and the
run's events (PR12's S9), so that `upstroke run` and `upstroke resume` print
and exit as they always have. The run's own `report.json`
(`engine::topology::finalize`, a `TopologyReport`) is written by the loop and
stays the authority; this is the command's summary of the same run.

- Tasks are the registry's entries, original and repair alike, in key order.
  A merged task is committed at the `merged_sha` of the `task_merged` that
  satisfied it; a failed one carries the failure of its last attempt that was
  not interrupted (an interrupted attempt returns its task to pending and never
  fails it, so a failed task's reason is read past one); a task an open
  question names is parked on it; anything else is queued, or skipped when the
  run halted or stopped at a budget — except that a pending task with a failed
  task in its transitive dependency closure is blocked ([`derived_blocked`])
  when the run completed, parked or has no `run_finished`.

  That exception is the canonical derived Blocked view
  (`decisions.run_end_policy.derived_outcome`, "Pending with a Failed task in
  its transitive dependency closure (derived Blocked)", and
  `decisions.repairs.terminal_failure`, "dependents Blocked as a derived
  view"). The durable state stays `Pending`: the fold keeps it, completes the
  run over it (`complete_shape`), and its own `report.json` still names it
  pending. Only this summary reads it. Until PR12-INV7-BLOCKED-TASK-REPORTED-QUEUED
  (first in `fffa73fa`) every such task at `Complete` was `Queued`, and
  `upstroke run` and `upstroke resume` ended a finished run by printing work
  that could never run as "queued", hiding why it had not. A halted or
  budget-stopped run keeps `Skipped` for every task it did not reach, failed
  ancestor or not, as it always has (the S9 halted guard,
  `engine::tests::a_halted_schema_4_run_projects_where_it_halted_and_its_outcome`,
  holds its failed task's dependent skipped); a pending task behind a parked
  one, or behind any task that has not failed, is still queued.
- Each task's attempts are the records the log holds for it, in log order:
  `attempt_finished` for an attempt that did not succeed, `candidate_prepared`
  for one that did, and `attempt_interrupted` for one whose coordinator did not
  see it end. The fold allows no other successful settlement (`check_attempt`),
  and `select::Spend::replay` counts the same first two events. So the trail,
  the model, the duration, the worker cost and the session are the legacy
  report's derivations over the same record type, and the review cost, the
  review models and the unknown-cost flag start from them.

  Until brief 3's S9 witness this read `attempt_finished` alone. A committed
  task then showed no model, duration or cost, and the total left out every
  successful attempt: the CLI's case F printed `total: $0.0000` beside a
  budget stop that recorded $0.02.
- An interrupted attempt is recorded with unknown spend (DESIGN §15, INV-14).
  `attempt_interrupted` carries no attempt record, so `interrupted_record`
  builds one from the `attempt_started` of the same identity: task key,
  generation and attempt number, which is how the fold names an attempt in
  flight (`check_attempt_interrupted`) and how `export` pairs a start with its
  settlement. The fold admits `attempt_interrupted` only for the identity in
  flight, so an interrupted attempt is never also finished or prepared and is
  counted once. Its record has no cost, so its task's `cost_incomplete`, and
  `RunReport::total_is_floor`, say the total is a floor, and its pool's drain
  row counts it as unpriced; nothing is added to the total and no price is
  invented, so the total still equals `Spend::replay`'s and no mismatch
  warning appears. An interrupted settlement whose start the log does not hold
  (a valid log has none) is still kept, naming no tier or model, and a warning
  says so.

  Until J12 (regular review 3, finding 1) this read `attempt_finished` and
  `candidate_prepared` alone. After a coordinator was killed mid-attempt and
  the run resumed, the report held only the attempts that completed, with no
  warning, and printed its known spend, `$0.04`, as the whole total.
- Every clone here copies a value the borrowed log or fold holds into the
  owned report: the report is an owned snapshot, returned to a caller that
  outlives neither input's borrow (§6).
- Merge verifications charge review passes that belong to no attempt record.
  `select::IntegrationReviews::replay` collects them from `merge_prepared`'s
  verification, a `merge_rejected` whose disposition is `CodeRejected`,
  `merge_verification_unavailable` and `merge_verification_charged`. It
  attributes each pass as `Spend::replay` charges it: to the task whose
  candidate the log names, or to no task for a terminal that no start pairs
  with. The fold admits a sequence's passes once (`check_spend_unrecorded`),
  so each pass is counted once:
  - a task's passes join its review cost, its review models and its
    unknown-cost flag (`review_cost_incomplete`);
  - an unattributed pass's known cost joins the total, and if one of them has
    no cost, a warning says the total is a floor;
  - every pass that names a pool counts in that pool's drain row: in
    `attempts`, with its known cost in `cost_usd` and an unknown one in
    `unpriced`.

  The total is the tasks' own totals plus the unattributed passes' known
  costs. A warning names any difference of more than 1e-9 from
  `Spend::replay`'s run total; the log is the authority.
  Until K4, `RunReport::total_is_floor` read the task rows only, so a total
  whose only unknown cost was an unattributed pass printed without its `?`,
  and only that warning said it was a floor. It also reads
  `unattributed_cost_incomplete` now. A valid log has no unattributed pass:
  the fold admits a verification's terminal only after its start.
- A merge verification that ended `merge_verification_interrupted` has
  unknown spend (DESIGN §26: the coordinator that died holding it recorded no
  cost for anything it was running, and what no record carries stays unknown
  spend). `IntegrationReviews` keeps the interruption against the task whose
  candidate it verified (K4, regular review 4, finding 4), and that task's
  `review_cost_incomplete` reads `IntegrationReviews::spend_unknown`, so its
  row and the total read as floors. The passes a `merge_verification_charged`
  recorded before the interruption stay in the task's review cost, the total
  and the pool drain, once each. An interruption no start pairs with sets
  `unattributed_cost_incomplete` and adds a warning saying so.

  Until K4 the replay ignored that terminal. After a coordinator died
  mid-verification and the candidate was verified again, the report printed
  the known charges as the whole total and `total_is_floor` was false, with a
  $2.50 charge and without one alike; the uncharged run printed `$0.0000`
  with no qualification (the reviewer's focused guards).

  Until J3 (`PR12-REPORT-OMITS-INTEGRATION-REVIEW-SPEND`) this read attempt
  records alone, so its review costs, its total and its pool drain all missed
  the review passes merge verifications charge.
- `halted_at` and `budget_stop` are set only as the fold's own outcome says,
  so [`RunReport::outcome`] agrees with it; the entry compares the two and
  warns if they ever disagree.

## `fn derived_blocked(`

The derived Blocked view of a schema-4 run, for [`RunReport::from_topology`]:
every task the fold holds `Pending` with a `Failed` task in its transitive
dependency closure, each mapped to the dependency it is blocked by.

The closure is the one the fold's completion rule computes
(`RunState::blocked_tasks`, behind `complete_shape`), from the same facts —
the registry's dependency keys (`TaskEntry::deps`) and the fold's task
states — read here through `TopologyFold::registry` and
`TopologyFold::task_state`, because the fold's own function is private to it.
It grows until a pass adds nothing, so a plan that lists a dependent before
the task it waits on is still closed.

The dependency it names is the first of the task's own dependencies, in
`deps` order, that is failed or itself blocked: the legacy [`settle`]'s
naming, so a direct dependent of a failed task names that task, and a
transitive one names the blocked dependency it waits on, whose own row names
the next link. The answer is the same whatever order the passes found the
closure in. A fold with no registry (no `run_started`) blocks nothing.

## `fn interrupted_record(`

The attempt record of an interrupted attempt, from its `attempt_interrupted`
and, when the log holds it, the `attempt_started` of the same identity. What
the start recorded is kept: the attempt number, the tier and model it was bound
to, its pool, and whether it resumed a session. What nobody observed is not
invented: no cost, no usage, no reviews and no session of its own, and a zero
duration, as the sequential engine's interrupted record has. Its failure is
`Interrupted`, from the worker, and its reason is the event's own detail, which
says why the attempt was settled interrupted (a coordinator that did not
survive it, or a halt that cancelled it).

It is built by `classify::attempt_record`, the one production construction of
an attempt's ledger line (`runner::contract`'s
`an_attempts_ledger_line_is_constructed_in_one_production_place`), classified
by `classify::interrupted_failure`. Until the J12 follow-up (regular review 3,
finding 1) it was a second `AttemptRecord` literal here, and that census failed
on it.

`attempt_record` reads `duration`, `cost_usd`, `session_id` and `usage` from the
outcome it is given, and `unobserved` gives it a zero duration and none of the
other three, so the record says what the literal said. Its `status`, `diff`,
`detail` and `transcript_path` are placeholders that `attempt_record` does not
read: they reach neither the record nor the report. `OutcomeStatus` has no
variant for an attempt nobody saw end, so `AgentError` there stands for nothing
observed. The feedback carrier is schema 4's, `AttemptRecord`, as the schema-4
driver's own settlement passes; the failure carries no feedback, so the
record's failure detail is empty under either carrier.

`AttemptFacts::tier` is a `crate::ir::Tier`, which has no unknown value. With no
start, the record is built with `Tier::Small` as a placeholder and its tier is
cleared before it is returned, so it names no tier the log does not hold; the
model is passed empty, the pool as none and the attempt as not resumed. That is
the narrowest place to remove the placeholder: `attempt_record`, `AttemptFacts`
and every other caller stay as they are, and a record with a start is exactly
what `attempt_record` built. The report's tests pin both records whole
(`an_interrupted_attempts_record_comes_from_the_one_record_builder_and_names_nothing_it_lacks`).

Its copies are the event's detail into the failure's owned reason, which
`attempt_record` copies again into the record, and the start's pool into the
owned record: owned snapshots of the borrowed log, as the rest of
`from_topology` takes (§6).

## `pub(super) struct ReportHeader<'a> {`

Everything a report needs that is not the plan or the state, kept together
so `build_report` stays readable at its call sites.

## `pub(super) running: bool,`

Whether an engine is driving this run right now.

## `pub(super) interrupted: bool,`

Whether this run stopped without ever recording that it finished.

## `pub(super) fn build_report(header: ReportHeader<'_>, plan: &Plan, state: &RunState) -> RunReport {`

The legacy report, from a header, the plan and a replayed state.

Each task position is read with checked access: the plan's task, its settled
state and its progress row. A position that one of them does not hold gets no
row and a warning that names it and says its spend, if any, is not in the
total, rather than a panic, a row dropped without a word or one invented to
fill the gap. A valid state has none: `RunState` holds one row per plan task,
and the log orders only positions it found by id. A function-local
`#[deny(clippy::indexing_slicing)]` holds the function to this (J17, regular
review 3, finding 5). Until J17 it read `plan.tasks[index]`,
`settled[index]` and `state.progress[index]`. Those reads were older than the
standard, and `questions_in_log` (a87d2439) brought the whole body under §7
when it changed the function.

The state's halt, questions and budget stop are cloned into the report, which
is an owned snapshot of a state the caller keeps (§6).

## `.chain((0..plan.tasks.len()).filter(|i| !state.order.contains(i)))`

Tasks that never started append in plan order, so the report reads
as the run happened and still accounts for everything.

## `let pool_drain = capacity::drain_of(state.progress.iter().flat_map(|p| p.records.iter()))`

§13's second currency: what each subscription drained, folded from the
same attempt records the dollar column comes from — so the two halves of
the ledger cannot disagree about the same attempt.

## `fn settle(plan: &Plan, states: &[TaskState], running: bool) -> Vec<TaskState> {`

Derive how an ended run's untouched tasks are reported.

This is a *view*, not state, and deliberately not recorded as events. A
task blocked behind an unanswered question has to become runnable again the
moment that question is answered — so if `Blocked` were folded in from the
log, every resume would have to un-fold it. Deriving it fresh from whatever
the log says is true right now means there is nothing to undo.

## `loop {`

Blocking propagates: a dependent of a blocked task is blocked too.
Repeat until stable rather than assuming plan order carries it — a plan
may list a dependent before the task it waits on.

## `if !running {`

Whatever is still Pending was never reached: the run halted. A run that
is still going has not halted — those tasks are queued, or one of them
is working right now — so leave them Pending for `task_report` to tell
apart.

## `fn blocks_dependents(state: &TaskState, running: bool) -> bool {`

Whether a dependency in this state will keep its dependents from ever
running.

`Blocked` means one thing to an operator — "a dependency failed, or parked
and was never answered" — and that is a claim about the future, not the
present. On an ended run the two coincide: anything short of `Done` is
final, because nothing more is coming. On a live one they do not. A
dependency that is merely pending, deferred, or in flight is a task whose
turn has not come, and its dependent is *queued behind* it rather than
blocked by it. Deciding this from `Done`-ness alone made `Queued`
unreachable for every task with a dependency, so the entire first half of a
live run read as a graph of failures.

## `TaskState::Pending | TaskState::Deferred => !running,`

Still on the way. Only an ended run turns that into "never".

## `TaskState::AwaitingInput(_)`

Terminal even mid-run, which is what keeps the propagation working
while the engine is still going: a parked dependency really does
block its dependents until somebody answers.

## `pub(super) fn last_reason(progress: &Progress) -> String {`

Why a task is parked or failed, for the report.

The most recent *attempt failure* wins, not the most recent feedback entry:
the branches that park a task never record feedback, so once an operator has
answered anything, their answer would otherwise shadow every later failure
and the report would tell them a task is parked because they answered a
question. Human entries are excluded from the fallback for the same reason.

## `TaskState::Deferred | TaskState::Pending => match &progress.in_flight {`

On an ended run, Deferred cannot survive `finish` and Pending is
settled away, so both mean the run stopped before this task got
its turn. On a live one `settle` leaves them alone, and the
attempt record says which of the two it is.

## `TaskState::Deferred | TaskState::Pending => match &progress.in_flight {`



## `TaskState::Deferred | TaskState::Pending => match &progress.in_flight {`

Every arm here is about a run that is still going, which is why
both are guarded. `Running` says of itself that only a live
`status` produces it, and a dangling `in_flight` on an ended run
is not a counter-example — it is an attempt whose engine died
between `attempt_started` and `attempt_finished`, which any error
out of `run_attempt` leaves behind. `finish` then wrote it into
`report.json` as `t1: running now — attempt 2 on mid` beside a
top-level `"running": false`: a stored document contradicting
itself, outliving the process that wrote it.

## `let mut seen: Vec<String> = Vec::new();`

Deduped, first-seen order: an escalated task can be judged by one
model on its first rung and another on the next, and both belong
beside a cost that counts both.

## `pub(super) fn total_of(tasks: &[TaskReport]) -> f64 {`

What every task cost, added up.

Deliberately not `Iterator::sum`, which folds floats from `-0.0`. That is
the *correct* additive identity in IEEE 754 — `-0.0 + x` preserves the sign
of `x` where `0.0 + x` does not — but it means the sum of no costs at all is
negative zero, and a run that has not yet spent anything rendered its ledger
as `total: $-0.0000`. Folding from `+0.0` cannot change a non-empty sum,
because the only value `+0.0` fails to preserve is `-0.0`, and a cost is
never that.

## `pub(super) fn sum_opt(values: impl Iterator<Item = Option<f64>>) -> Option<f64> {`

Sum, preserving "nothing was reported" as `None` rather than `0.0` — a
ledger that cannot tell free from unreported is worse than no ledger.

## `pub fn topo_order(plan: &Plan) -> Vec<usize> {`

Stable topological order: among ready tasks, lowest plan index first (§14).
Used for previews and reporting; the live scheduler derives readiness per
step instead, so parked work can be skipped past.

## `for (i, flag) in done.iter_mut().enumerate() {`

Unreachable on a validated plan; degrade to plan order.

## `fn switched_the_checkout(&self) -> bool {`

Whether this run checked its branch out over the operator's checkout, which
is what the summary's `git switch -` returns from. A schema-1..3 run does: the
coordinator's `create_branch` switches to the run branch when it starts, and a
resume's `switch_branch` does when the operator is elsewhere. A schema-4 run
never does: it integrates onto its branch through refs and leaves the user's
checkout untouched (DESIGN §14, INV-12), so a `git switch -` printed after it
moves the operator to whatever branch they last left, or fails when there is
none (PR12-REG6-TOPOLOGY-RETURN-HINT; the regression lens read the line in the
C1 and C2 cell streams).

The distinction is the existing projection one rather than a new field:
`from_topology`, the one projection of a schema-4 run, is the only construction
that sets `questions_in_log`, and every legacy one — `build_report`,
`from_state`, a deserialized `report.json` — leaves it false. A field of its
own would change the serialized `RunReport` and every literal that spells it,
for a fact the projection already carries.

## `pub fn render(&self) -> String {`

Human-readable report with one sanitized payload per layout line.
Control characters in recorded fields cannot add terminal commands or
lines. Rendering leaves the report's serializable values unchanged.

The branch line keeps `(return with: git switch -)` only for a run that
switched the checkout ([`switched_the_checkout`]); a schema-4 summary, fresh
or resumed, names the branch alone. The legacy wording is unchanged.

## `let partial = if task.review_cost_incomplete { "?" } else { "" };`

`?` marks a total with unreported components — the
Copilot route bills nothing back, so a two-pass review
shows one reviewer's spend and must not read as both.

## `([], _) if task.review_cost_incomplete => " + review $?".to_owned(),`

A committed task with no review pass on record whose review spend is still
unknown: its candidate's verification was interrupted before any pass was
charged (K4). Saying nothing would read as an unreviewed, free task.

## `(models, None) => format!(" + review {} $?", models.join(", ")),`

Reviewed only by routes that report no spend (§13) —
say who judged it rather than imply it was free.

## `let worker = match task.cost_usd {`

Same rule as the reviewer half beside it, which has said
`$?` since step 9: a route that reports no spend has not
reported zero. `unwrap_or(0.0)` printed `$0.0000` for a
codex-implemented task while the ledger three lines below
correctly showed `—`, so one run said both.

## `let ending = if self.interrupted {`

Why it never got its turn, since the two endings are not
the same thing to an operator: a halt is a decision the
run reached, an interruption is one that happened to it
and that `resume` undoes.

## `TaskRunStatus::Unknown => {`

Only reachable from a `report.json` written by a newer
upstroke. Say that, rather than picking a familiar-looking
status and being confidently wrong about someone's run.

## `if self.running {`

A live run has no outcome yet, and every arm below claims one. Say
what is true instead: how far it has got.

## `if self.interrupted {`

Neither has a run that stopped without recording a finish, and for
the same reason: there is no outcome to report yet. `outcome()`
cannot see that — a killed run has nothing halted, no budget stop and
nothing parked, which reads as `Complete` — so it used to print `run
complete: N task(s) committed` about a run that died mid-attempt,
one line above `status`'s own `state: interrupted`.

## `if self.interrupted {`



## `if self.interrupted {`

"So far" is the live line's word on purpose: more may yet come, once
somebody resumes. Which is also why the resume command is not
repeated here — the `state:` line in `status` already carries it, and
saying it twice invites the two copies to drift.

## `let stopped = self.budget_stop.as_ref().map_or_else(`

`outcome()` only returns this when `budget_stop` is set, so
the fallback is unreachable — and it says so rather than
naming a plausible ceiling. A specific, checkable, false
claim about the operator's own config is the worst thing to
print here.

## `pub fn render_ledger(&self) -> String {`

§21's definition-of-done (e): what each task cost, and on what.

Implementer and reviewer spend stay in separate columns because they
are different models at different tiers — folding them together makes a
cheap rung look expensive to anyone reading the ledger (§13). An
unreported cost prints as `—` rather than `$0.0000`: a ledger that
cannot tell free from unreported is worse than no ledger.
Cell text is sanitized before measuring widths; row payloads pass
through the same terminal boundary as the report.

## `let partial = |rendered: String, incomplete: bool| {`

A figure that omits a reviewer whose route bills nothing back is not
the total, and this column is where someone decides what a run cost.

## `let rows: Vec<[String; 6]> = self`

Own the formatted cell snapshots so widths are measured from the
same sanitized text that the second pass emits.

## `if self.pool_drain.is_empty() {`

§13's second currency. An empty section means no attempt in this run
named a pool — which is the honest reading of "no pools connected",
and is said rather than left as a blank column that looks like
"nothing was spent".

## `None => "— (this route reports no spend)".to_owned(),`

Every attempt on this pool ran on a route that reports no
spend (§13) — saying "$0.0000" would read as free.

## ``"  stopped by [budgets] {} = ${:.4} before `{}` (§13)",``

The ledger annotates; `render` owns the outcome line and the
resume advice. Printing both put two near-identical
paragraphs, formatted to different precision, with two copies
of the same command, back to back in `upstroke status` — which
reads as two things having happened.
