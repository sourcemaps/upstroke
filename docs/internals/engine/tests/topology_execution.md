# `src/engine/tests/topology_execution.rs`

Extended notes for [`src/engine/tests/topology_execution.rs`](../../../../src/engine/tests/topology_execution.rs).

## Module

The allowlist-placement marker stays in source above the governed lint allowance;
the effects census requires it there.

## `fn the_topology_stubs_edit_appends_one_line_per_run_under_the_native_shell() {`

The stub's edit is a real append under the shell `shell_spec` picks on this
platform, run by the production `HostRunner`: each run adds one line, so a
task whose base already holds an earlier task's output still leaves a diff
of its own. A second task's empty diff on the Windows guest (CI run
41529099549) is what an edit that left the file as it was looks like. Only
a Windows host runs the edit under `cmd`, so only there does this
discriminate between the spellings.

## `fn the_topology_stub_completes_only_a_worker_that_exited_cleanly() {`

The stub reads a worker's end as a real adapter does: only a clean exit
completes, a timeout is a timeout, and any other end is an agent error, so
a failed edit is reported with its own exit and stderr, not as an empty
diff. This holds and discriminates on every platform.

## `fn overlap_observing_worker(markers: &Path) -> String {`

A worker that notes, for about five seconds, whether another stub worker is
running beside it, then writes a file of its own and leaves. `overlap.*` in
`markers` is the evidence two workers ran at once. Each attempt writes a
different file, so two candidates never conflict at the merge.

Five seconds, not one: in V139's full suite, beside a guest VM, two
dispatches at `max_parallel = 2` started more than a second apart, and the
premise that two workers ran at once failed, while the test alone passed
(V140). A longer look makes the premise robust to load, and it gives the
bounded arms (`max_per_agent = 1`, a one-slot pool) longer in which a
forbidden overlap would be seen, so their zero-overlap assertions are only
stronger for it.

## `fn fresh_overlap(tag: &str, extra: &str, pools: Option<&str>) -> usize {`

A fresh run at `max_parallel = 2` over two independent tasks, under `[engine]`
`extra`: whether two workers overlapped.

## `fn resumed_overlap(tag: &str, extra: &str) -> usize {`

A schema-4 run whose two tasks both park on a question, resumed with both
answers delivered under today's `[engine]` `extra`: whether the resumed
workers overlapped. The answers are written where the run's own answer
source polls for them before every selection, so both tasks are ready
together when the resumed loop starts.

## `fn a_schema_4_run_publishes_its_records_in_order_and_records_one_runner() {`

P12-W1's rest and P12-W5. Through `engine::run_with`: the owner record was
published before the first probe (P3b before P4); its runner is the one
`run_started(4)` records, byte for byte; the commit record names the digest of
the committed first line; the log begins with `run_started` and ends with
`run_finished`; the integration ref holds the last merge; and `report.json` is
the projection of the committed log.

## `fn at_width_one_the_step_loop_and_the_concurrent_driver_project_alike() {`

P12-W14 (D4). At width 1 the production composition through the step loop
(an adapter source that cannot be shared) and through `run_concurrently` (one
that can) give the same canonical projection of the same plan.

## `fn a_schema_4_run_ended_by_a_signal_mid_attempt_resumes_and_settles_the_attempt() {`

P12-W2 and P12-W16 (D6): a schema-4 run whose process dies by SIGKILL,
SIGTERM or SIGINT inside an attempt leaves a log that ends mid-attempt; the
resume settles that attempt as interrupted, appends `run_resumed`, and
continues to the end. No graceful drain is claimed: the signal ends the
command.

## `enum SignalFault {`

How [`signal_mid_attempt`] is made to fail, for the tests of its own cleanup. Each
fault is injected once the helper's worker runs. `Readiness` reports the readiness
the driver waited for as failed, and `Signal` the signal it sent, as a real failure
of either would be (J8, PR12 regression review #5). `DeliveryNeverExits` makes the
command that delivers the signal one that never exits: a stub standing where `kill`
stands, which records its pid and then sleeps for ten minutes (J15, regression
review 2, finding 4). `None` injects nothing.

## `fn worker_pids(repo: &Path) -> Vec<u32> {`

The pids the signal helper's workers wrote, one a line. Each leads a process group of its
own: the process funnel puts every agent it starts in one.

## `fn stalled_delivery_pid(repo: &Path) -> PathBuf {`

Where the stub of `SignalFault::DeliveryNeverExits` records its pid, beside the
repository, published by a rename. The J15 test reads it after the driver has
failed, to find that process gone.

## `fn delivery_program(repo: &Path, fault: SignalFault) -> PathBuf {`

The program that delivers the signal: `kill`, found on `PATH` as before, or, for
`SignalFault::DeliveryNeverExits`, the stub, written beside the repository by a
process of its own (`runner::container::write_program_in_its_own_process`), so that
no other thread of this test binary holds it open for writing when it is executed.

## `fn deliver_within_its_bound(program: &Path, signal: &str, pid: &str, repo: &Path) -> bool {`

Delivers `signal` to `pid` by running `program -s <signal> <pid>` as a
[`LinkedChild`](crate::workspace_manager::fixture::LinkedChild)
(`LinkedChild::spawn_program`) and waiting for it with `wait_within(COLLECT_BOUND)`,
the bound the helper's own end has. A delivery still running at the bound fails the
driver, naming the command; as the failure unwinds, the link's drop kills and reaps
the command within its own bound, so it outlives neither the driver nor the test.
`true` when the command exited successfully.

## `fn signal_mid_attempt(`

The signal test's driver. The helper, this test binary running
`topology_signalled_child`, is owned from its spawn by a
[`LinkedChild`](crate::workspace_manager::fixture::LinkedChild), which kills and
reaps it within its bound however this function ends. Every wait the driver makes
is bounded: the readiness poll, the delivery of the signal
([`deliver_within_its_bound`]), the helper's end and its cleanup, its reader's
settlement, each worker group's end and the run's release. At the reviewed head of
J8 the helper was a bare `Child`: an assertion before its unbounded `wait` unwound
past it and left the helper and its worker running (J8, PR12 regression review #5).
`helper` learns the helper's pid as it is spawned.

J15 (regression review 2, finding 4): J8 left one wait unbounded, and said so. The
`kill` that delivers the signal was a `Command` whose `.status()` had no deadline:
a `kill` that never exited held the driver inside that call, before either the
delivery's assertion or `wait_within(COLLECT_BOUND)` could run, and the test and
the helper it owned stayed live indefinitely. §12 binds every wait, and a
disclosure does not discharge that. The delivery is now itself a link, waited for
within `COLLECT_BOUND` ([`deliver_within_its_bound`]); `COLLECT_BOUND` and every
assertion are kept, the J12 ones below included.

J12 (regular review 3, finding 1): after its order assertions, the driver
asserts that the resumed run's report says its total is a floor
(`RunReport::total_is_floor`), and that it keeps exactly one interrupted attempt,
whose cost is unknown and whose model is the one recorded by the
`attempt_started` with the same task key, generation and attempt as the
`attempt_interrupted` the resume appended. At the reviewed head the report held
only the attempts that completed, with no warning, and printed the run's known
spend, `$0.04`, as its whole total.

## `fn the_signal_tests_helper_is_settled_when_its_readiness_or_its_signal_fails() {`

J8, PR12 regression review #5: the signal test owns its helper from the spawn, so neither a
readiness that fails nor a signal that cannot be delivered leaves the helper, or the worker
it started, running. The reviewer's witness was a `kill` that exited 1: the parent failed
at its assertion, and seven of the helper's processes outlived it. Each fault is injected
into the driver once the helper's worker runs. The driver fails as the test would, and
then the helper has been reaped, the worker's group is gone, and nothing holds the run,
each within its bound.

## `fn the_signal_tests_helper_is_settled_when_the_command_delivering_its_signal_never_exits() {`

J15 (regression review 2, finding 4), the reviewer's guard: the signal's delivery
is the stub of `SignalFault::DeliveryNeverExits`, a command that never exits. The
driver fails, within its bound, naming the delivery that had not exited. Afterwards
the stub, at the pid it recorded, is gone (its link's drop killed and reaped it);
the helper is reaped; each worker's process group is gone within `GROUP_END_BOUND`;
and nothing holds the run.

Against the driver as J8 left it, with the delivery's `.status()` unbounded, the
test does not end: measured once, it was still running past libtest's sixty-second
notice, and the deadline put on it from outside (a session of its own under
`timeout`, 120 seconds) killed it; nothing it started outlived the session, and
the scratch tree its unwinding would have reclaimed was removed by hand.

## `fn a_schema_4_question_is_announced_before_its_sibling_is_released() {`

J11 (regression review 2, finding 2): a question a schema-4 run raises is
announced through the configured notifier while an independent task is still
held, under both drivers and after a resume. Four cells: a fresh run and a
resumed one, each under the step loop (width 1, a source that cannot be shared)
and the coordinator (width 2). The config is `mode = "never"` and `notify =
["cli"]`. For a resumed cell the first run, in this process, stops at a $0.015
budget once `first` has spent, before `asks` or `waits` runs, and the child
resumes it with a $10 budget.

The run itself is a child of this test binary (`topology_announcing_child`)
because the CLI notifier writes to the process's stderr, which the child sends
to a file. The test reads that file until it holds the notifier's line for
`asks` (`question … — parking asks; the run continues`) and `waits-started`
exists. Only then does it release `waits`, which the run cannot finish without,
so the announcement is shown to arrive before the sibling is released and while
the run is still going. The child must then end successfully. Under the step
loop `asks` runs before `waits` by key order, so the announcement precedes the
sibling's start; under the coordinator the two run together.

At the reviewed head nothing was announced: with the announcement removed from
`RunEmitter::emit`, the first cell fails at its 90-second bound with an empty
stderr.

## `fn a_schema_4_status_reads_the_committed_prefix_and_leaves_the_log_as_it_found_it() {`

PR12 S10: the read commands over a schema-4 log (D1, D3, D10).

P12-W12. `status` on a schema-4 run reads the committed prefix only: its
projection equals the barrier's replay and the run's own `report.json`; with
a writer holding the run and a torn tail at the end of the log, it reports
both, names the runner identity, and leaves the log's bytes and mtime as it
found them; `--follow` prints the run's events and stops once it finished.

## `fn a_schema_4_follow_prints_one_line_per_event_and_no_raw_control_whatever_the_plan_or_the_record_holds()`

L1 (regression review 3, finding 1; DESIGN §18). A schema-4 run of one task
annotated `id=t1` followed by ESC `[2J` completes, and `status --follow`, through
the public `status::load` and `status::follow`, prints exactly one physical line
per committed event, with no control character but the line breaks, and the
`attempt_started` and `task_merged` lines name the task as `t1\u{1b}[2J`. Then
`Observed::describe` is given that run's `attempt_started` with controls in
three recorded fields, its timestamp, its agent and its model: the line holds no
control, and the agent and model appear with theirs made visible. With
`describe`'s `one_line` removed, the follow assertion fails on the raw ESC bytes.

## `fn a_fresh_schema_4_run_refuses_a_staged_an_unstaged_or_an_untracked_change_before_any_worker_or_log()`

L2 (regression review 3, finding 2; DESIGN §20). Through the production entry
(`run_with` with the topology active), three repositories with one kind of dirt
each: a staged edit of `README.md`, an unstaged edit of it, and an untracked
file. Each run is refused with "working tree is not clean", before any worker or
reviewer is built (`ObservedStub` records none) and before any run directory, and
so any run log, exists; the operator's change is untouched. The control: one
clean repository is admitted twice in a row, its workers running each time, so
the first run's public directory does not make the second run's tree dirty.
Measured before commit: with the schema-4 dispatch's refusal removed the staged
case runs to completion and the test fails; with the exclusion removed from
`refuse_dirty_tree` the second clean run is refused and the test fails.

## `fn cut_log_after(`

A coordinator that died just after appending an event leaves exactly the committed prefix that
ends with it, because the log only grows at its end. This cuts a finished run's log back to that
prefix after the first committed line whose event is `kind` and satisfies `matches`. The run is
released before the cut, so `status` reads it as interrupted, not running.

## `fn upstroke_status(repo: &Path, run_id: &str) -> String {`

What `upstroke status <run>` prints, through the public `status::load` and `status::render`.

## `fn ledger_cells(status: &str, id: &str) -> Vec<String> {`

The cells of the ledger row `status` prints for one task, split on whitespace. A trail with
several words spreads over several cells, so callers count the amounts from the end.

## `fn a_schema_4_status_prints_the_ledger_run_reports_for_a_completed_and_an_interrupted_run() {`

L3 (regression review 3, finding 3; DESIGN §18). Two dependent tasks on one pool, completed.
- `status` prints, byte for byte, the ledger the run returned (`RunReport::render_ledger`): the
  task rows, the known total, the per-pool drain.
- Both fast integration rows end `(0 review(s), cost none)`.

The same log cut just after the second task's `attempt_started`:
- `status` reads the run as interrupted. The first task's row shows its exact known spend.
- The second task's row counts the cut-off attempt and knows no amount (`—`).
- The total and the pool drain carry `?`, and the floor legend is printed.

Measured before commit, each mutation failing this test:
- with the ledger removed from `Observed::render`, on the completed comparison;
- with the interrupted view's settlement removed, on the cut-off row (`t2 0 — — — —`, no floor);
- with `ran_no_review` removed, on the fast rows.

## `fn a_schema_4_status_prints_the_integration_review_spend_run_reports_and_its_floor_once_cut_off()`

L3, the integration reviews (Unix: the worker writes a file named by its own process). Two
independent tasks at width 2 make the second merge a stale candidate's re-verification, with one
priced review pass that the verified task pays for. Completed:
- `status` prints the ledger the run returned; the verified task's review cell includes that
  pass.
- The `stale_clean` row ends `(1 review(s), cost $X)` and the fast row `(0 review(s), cost none)`.

The log cut just after `merge_verification_started`:
- The verified task's review cell is its attempt's known pass with `?`, and the run's total is a
  floor.
- The integration row is `open` with `cost ? (unknown)`.

The ledger and settlement mutations above fail it too.

## `fn a_completed_schema_4_run_leaves_no_generated_file_for_an_ordinary_git_add_to_take() {`

L4 (regression review 3, finding 4). A real schema-4 run, through `run_with`, completes in a
repository that has no `.upstroke` exclusion before it (the premise is asserted). Its public
directory holds the run's log, frozen plan, report and locks. Afterwards `git status` lists nothing,
and an ordinary `git add -A` stages nothing (`git diff --cached --name-only` is empty). With the
exclusion removed from `coordinator::refuse_dirty_tree`, it fails on `?? .upstroke/`.

## `fn a_schema_4_resume_restores_a_missing_run_exclusion() {`

L4, the resume. A schema-4 run parks on its one question, the answer is recorded, and the
exclusion its fresh run made is deleted. The premise is asserted: `git status` now lists
`.upstroke/`. The resume, through `resume_with`, completes the run and restores the exclusion, as
the sequential resume does. Afterwards `git status` lists nothing. With the schema-4 branch's
`ensure_run_exclusions` removed from `resume_harness`, it fails on "the resume made the exclusion
again".

## `fn a_budget_stopped_schema_4_resume_recreates_its_pruned_execution_root_where_it_recorded_it() {`

The positive control of PR12-INV6-RESUME-COMMON-DIR-REBIND for a root that finalization legitimately
pruned, on every platform. A run stopped at its budget (S9's fixture: one attempt per task, a
`0.015` run budget) is closed and finalized, and its emptied execution root is pruned; the premise
reads the recorded root absent. Resumed with a larger budget under the same Git environment, the
run is admitted although its root does not exist — the binding compares paths and requires no
existence — and recovery recreates the root, saying so ("the execution root was pruned when the run
last ended and has been recreated"), with no other namespace made under the private root.

## `fn settled_attempts(events: &[serde_json::Value]) -> usize {`

How many attempts the log settles: `attempt_finished` for each one that did not
succeed, `candidate_prepared` for each one that did, and `attempt_interrupted`
for each one whose coordinator did not see it end. Since J12 the schema-4 report
keeps the last kind too, so the tests that compare the report's attempt rows
with this count stay a comparison with every settlement.

## `fn display_of(projected: &crate::engine::topology::report::TopologyReport, key: u64) -> String {`

The display id finalize's projection gives a task key.

## `fn a_halted_schema_4_run_projects_where_it_halted_and_its_outcome() {`

S9, a halted run. A task whose gate fails on its one attempt reaches the end of
its ladder, and the run asks before failing it: it parks on the task's
question (V109 found this, against the first draft's premise of a run that
halts at once). The operator gives the task up through the public `answer
--decline`, and under the default `on_task_failure = halt` the resume halts.
The `RunReport` that `engine::resume_with` returns projects what the fold
says: `outcome()` is `Halted`, `halted_at` names the task the log's
`run_finished` names, the task behind it is skipped, and nothing claims a
budget stop. A fresh observation of the log and finalize's `report.json` say
the same. `main.rs` turns this outcome into its exit status; the command-line
scenario's case E runs that.

## `fn blocked_closure_plan() -> String {`

Four tasks: `t1`, whose body carries [`ESCALATES`] so its workers below the frontier end without an
edit; `t2` on `t1`; `t3` on `t2`; and `t4` on nothing.

## `fn dependent_rows(rendered: &str) -> Vec<&str> {`

The rendered rows of `t2` and `t3`, the direct and the transitive dependent.

## `fn declined_failure_run(repo: &Path, stub: &ObservedStub) -> RunReport {`

A schema-4 run through the public `run_harness` whose end-of-ladder question is declined by the
harness's scripted answer, so `t1` fails after its one attempt, under whatever `on_task_failure`
the repository's config holds.

## `fn a_completed_schema_4_run_reports_its_failed_tasks_dependents_blocked_live_and_replayed() {`

PR12-INV7-BLOCKED-TASK-REPORTED-QUEUED. With `on_task_failure = "continue"` the run completes with
`t1` failed and `t4` committed, and its final summary prints ``t2: blocked by `t1` `` and
``t3: blocked by `t2` ``, never `queued`: the derived Blocked view of the canonical completion rule.
The run's committed log, replayed through the fold's checked transitions with the frozen plan read
as `entry::frozen_inputs` reads it, is `Complete` with `t2` and `t3` still structurally `Pending`
— the run's own `report.json` says `pending` too — and projects the same rows. On the same log, the
prefix before the event that fails `t1` reports them `queued`, and the prefix ending with it
reports them blocked: that event, and nothing else, is what blocks them.

## `fn a_schema_4_report_keeps_queued_behind_a_parked_task_and_skipped_behind_a_halting_failure() {`

The controls. A task behind one parked on an unanswered question is `queued`: a question is not a
failure. And the same failing chain under the default `on_task_failure = halt` ends `Halted` at
`t1`, with `t2` and `t3` `skipped (run halted)`: a halted run keeps skipping what it did not reach.

## `fn a_schema_4_run_stopped_at_its_budget_projects_the_stop_and_its_outcome() {`

S9, a budget stop. A run budget the first task's spend crosses stops the run
before the next attempt. The `RunReport` projects the stop the log records:
`outcome()` is `BudgetExceeded`, and `budget_stop` carries the log's
`budget_exceeded` budget, limit and spend, and the task it stopped at, by its
display id. Nothing claims a halt, and a fresh observation of the log and
finalize's `report.json` say the same. `main.rs`'s exit status 3 is the
command-line scenario's case F.

## `fn a_task_budget_stop_projects_the_task_it_names() {`

S9, a task-budget stop. `[budgets] task_usd` refuses a task's retry once the
task's own spend reaches it, and the log's `budget_exceeded` names that
task's key. The `RunReport` projects the stop to the task's display id. The
run-wide stop above, refused at integration, names no task and projects to
an empty one; both semantics stand side by side.

The gate fails the first candidate that carries the stub's output, once,
so the task's first attempt is spent and its retry is what the task budget
refuses.

## `fn a_fresh_run_records_the_case_policy_and_a_resume_never_changes_it() {`

P12-W11 (D7). A public fresh run records the case-fold resolver's answer in
`run_started(4)`'s `path_policy`, and a resume never changes it, even where
the resolver would now answer differently. The managed repository's
`core.ignorecase` changes between each run's creation and its resume, and a
fresh run made after each change shows what the resolver answers then.

The namespace is observed the way the resolver observes it (one name, looked
up in the other case, beside the private root). Where it folds, every answer
is `true`, so only "a resume never changes it" can be shown there.

On a case-sensitive namespace, the run that recorded `true` resumes over a
root that does not fold. The resume continues on the record and warns (D7's
resume side), while the run that recorded `false` resumes without a word.

## `fn a_schema_4_run_integrates_where_git_has_no_committer_identity() {`

FINDINGS2_3, the committer-identity parity check (root's checkpoint-3
disposition). At `b9d15bdb` the legacy engine asked the operator for no
identity: every commit it put on a run's branch was `commit-tree` with
`upstroke`'s (`Workspace::commit_tree_with_upstroke_identity`). The schema-4
integration's proposal cherry-pick took the committer from the operator's
configuration, so a repository without one stopped there (the CLI
scenario's case C at `ad513bdf`). That was an activation regression, and the
cherry-pick now carries the engine's committer identity.

The child runs with every ambient Git control taken away. In it, the
repository's own identity is removed and `user.useConfigOnly` is set, so Git
can find no committer identity at all, which is checked first. Two
independent tasks at width 2 make the second merge a stale candidate's
cherry-pick, which is checked too.

## `const THREE_INDEPENDENT_TASKS: &str`

Three tasks in flight from one base. The two in slots `k0` and `k1` both
create `shared.txt` with different contents, and the one in `k2` writes a file
of its own: whichever merges first, a later candidate is stale and clean (a
merge verification) and another conflicts (a merge rejection, which spawns a
merge-repair task).

## `fn a_schema_4_export_keeps_todays_rows_and_joins_a_repair_task_to_the_registry() {`

P12-W18. `export-decisions` over a schema-4 run with a merge-repair task:
one row per attempt, in today's columns, the repair task's attempts joined to
the frozen registry by its display id, and no verification or merge event as
a row. Each row is settled as the log settles its attempt: one that
succeeded, by `candidate_prepared`, is `passed`, with its finish and cost.

## `fn a_schema_4_report_counts_every_integration_review_once() {`

J3, `PR12-REPORT-OMITS-INTEGRATION-REVIEW-SPEND` (both reviews). Three tasks in flight from
one base: a later candidate is stale and clean, so its integration verification runs a
review the run's spend ledger charges. At the reviewed head the CLI report collected
attempt records only and its total missed that charge ($0.10 against $0.11 recorded; the
second witness, $0.0400 against $0.0500). The report now carries those passes on the
candidate's task, once, and its total, and the tasks' own totals, are the ledger's, with no
mismatch warning.

## `fn a_schema_4_resume_runs_the_recorded_gates_and_names_how_todays_differ() {`

J7, PR12 regression review #4: a schema-4 resume runs the gates its run recorded and says
how today's config differs, as `resume_runs_the_gates_the_run_recorded_not_todays` pins for
the sequential engine (DESIGN §17). At the reviewed head the resume restored the recorded
gates without comparing them, so an edited gate passed unremarked. Today's config here edits
one gate's command and another's timeout, renames a third, and changes the shell: the
warning names every edit, and the recorded gates, not today's failing command, decide the
run.
