# `src/engine/tests/capacity_budget.rs`

Extended notes for [`src/engine/tests/capacity_budget.rs`](../../../../src/engine/tests/capacity_budget.rs).

## `const CLAUDE_UNRELATED_RESET: &str = r#"{"type":"result","is_error":true,"result":"MCP cache resets 9am (UTC)","session_id":"s-limit"}"#;`

A failed result that signals no rate limit but names a reset of its own:
`MCP cache resets 9am (UTC)`. It has no `hit your … limit` line, so its
reset is no limit's. When the parser still read a text's only `resets`,
reading it took 09:00; now it gives `None`, and the tests that carry it
check that no 09:00 is recorded from it whatever the stderr says.

## `Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),`

A single attempt on a single rung: if the rate limit spent it,
the task could never commit.

## `fn pools_file(repo: &Path, content: &str) -> PathBuf {`

---- step 10: pools, budgets, and spend approval (§13) ------------------
A pools file beside the repo — never `~/.upstroke`, which is the
operator's, and never inside the workspace, where §14's `git clean -fd`
would delete it.

## `let (_tree, repo) = temp_engine_repo("budgetstop");`

The one-fold property, on the branch step 10 added: the stop is an
event, `RunState::apply` is what turns it into state, and a replay of
the log lands on the same state the live run held.

## `assert_eq!(report.outcome(), RunOutcome::BudgetExceeded, "{report:?}");`

Each task costs 0.06 (0.01 implementer + 0.05 review), so the ceiling
is crossed after the first and the second task is refused before it
spawns anything.

## `let events = events_of(&repo, &report.run_id);`

Exactly once: the scheduler stops scheduling on the first stop, so a
second would describe a spawn that never happened.

## `assert!(matches!(task(&report, "t2").status, TaskRunStatus::Skipped));`

Nothing after t1 ran, and the untouched tasks settle as skipped.

## `let source = source(`

Fails on the first rung, so a second attempt is asked for — and
refused, because this task has already spent past its own ceiling.

## `let (_tree, repo) = temp_engine_repo("budgetresume");`

D4's whole point: a budget stop is recoverable in one command,
because budgets are re-derived at resume rather than inherited.

## `let (_tree, repo) = temp_engine_repo("approvespend");`

D3, end to end. The engine escalates FIRST and then asks, so an
approved task un-parks already standing on the frontier rung with a
fresh allowance — and `answer_question` needs no ApproveSpend arm.

## `let tiers: Vec<&str> = task(&report, "t1")`

The approved attempt really ran on the frontier rung with the
allowance the escalation reset — not a re-run of the mid rung.

## `assert_eq!(report.outcome(), RunOutcome::Halted, "{report:?}");`

Through `ingest_answer`'s existing Declined path — the one place that
owns the halt policy, with no ApproveSpend special case beside it.

## `let (_tree, repo) = temp_engine_repo("frontierstart");`

§12's target is silent escalation. A task the operator deliberately
routed to frontier in config was not escalated onto it silently, and
asking anyway trains people to approve without reading.

## `let drain = &report.pool_drain;`

§13's second currency in the ledger, folded from the same records the
dollar column comes from.

## `let events = events_of(&repo, &report.run_id);`

And §14's pre-flight snapshot is on the record — folding to nothing,
which `assert_live_equals_replay` elsewhere is what proves.

## `fn a_legacy_run_refuses_a_profiled_pool_at_preflight_rather_than_ignore_it() {`

The legacy coordinator does not carry a pool's `profile`: only runs created on
the execution topology (schema 4) stamp it onto their agents' requests. So a
legacy run whose pools name one is refused at pre-flight
(`preflight::refuse_profiled_pools`), naming the pool, the profile and where
profiles apply, before the run branch or a run directory exists. Without the
refusal it would run every attempt under the operator's default credentials
while the pools file says otherwise. A fresh run is the case driven here; a
legacy resume reaches the same check, because both go through
`preflight_with_recorded`.

## `let (_tree, repo) = temp_engine_repo("poolexhausted");`

§13 source 1 made real: the signal is ground truth, and the estimator
that reads it back must never let a self-metered figure talk it up.

## `let signal_at = events`

A fold that stops at the signal reads the pool as exhausted, at the
top confidence rank — the signal is ground truth about that moment.

## `let settled = capacity::estimate(&cfg.pools, &capacity::observe(&events));`

But the whole log has the pool serving an attempt afterwards, so the
signal is retired rather than standing forever. Reporting `exhausted`
here — on the same line that reports the attempts it served — was the
shape the review caught.

## `let (_tree, repo) = temp_engine_repo("budgetflag");`

`[budgets] run_usd = 0.0` is a hard error at load. The flag that
overrides it must not be a way around that: zero and negative both
stopped the run before it spent anything, and NaN silently never
fired at all.

## `let branch = git_in(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]);`

And refused at pre-flight, before a branch or a run directory exists.

## `let (_tree, repo) = temp_engine_repo("approvalfeedback");`

Every other question's answer is guidance for the next attempt. An
ApproveSpend answer is a yes/no about money whose meaning was already
consumed by the un-park, and `feedback_section` frames feedback as
"an instruction from a person… it takes precedence over your earlier
assumptions" — which is not a thing to tell a coding agent about a
billing decision.

## `assert!(`

An Unblock answer still does, because there it really is guidance.

## `let (_tree, repo) = temp_engine_repo("cannedoption");`

The options a question carries are the engine's instructions to the
operator: "retry this task with guidance you type below", "answer in
your own words". `upstroke answer <id> --option 1` resolved to that
sentence and pushed it as human feedback — so it reached the
implementer framed as "an instruction from a person", and once §12's
decisions were routed to the judge as well, it reached the reviewer
as "a decision from a person… a change that departs from it is a
defect however well argued". There is no diff that satisfies a
sentence about where to type, so an honest judge rejects every
attempt until the ladder is spent.

## `let source = source(`

Ask, then answer by picking the first option verbatim — what
`--option 1` writes.

## `let runs = source.adapter.runs();`

The retry happened — the answer still un-parks the task.

## `for review in runs`

And nothing reached the judge as an operator decision either.

## `let source = source(`

Down for three attempts, then back.

## `let (_tree, repo) = temp_engine_repo("budgetdirty");`

§14 keeps the working tree for a resumed same-rung retry, because
that retry re-gates the *cumulative* diff. The ceiling is checked at
the top of the same loop, so a budget reached between the two
returns to the operator with a rejected attempt's edits still staged
in their repository — and staged changes follow `git switch` onto
whatever branch is visited next. Observed on a real repository:
run 01KZNMR59E5ATC9MBYY29WZB6E left two files staged after exit 3.

Keeping them buys nothing even in principle: `run_resumed` discards
every uncommitted path and clears `session`/`resume_next` on every
task, so the retry those edits were preserved for cannot use them.

## `opts.budget_usd = Some(0.05);`

Enough for the first attempt, not for the retry that attempt asks
for — so the stop lands exactly between the two.

## `let mut resume_opts = resume_options(&repo, &stopped.run_id);`

And the run is still exactly as resumable as it was before.

## `fn priced_and_unpriced_attempts() -> TaskReport {`

One attempt that reported its spend and one that did not — the shape a
kill/resume leaves, and a mixed-route ladder too.

## `fn empty_report() -> RunReport {`

A `RunReport` with nothing in it, for tests that care about one field.

## `let mut task = task_report_costing(None, None);`

§13's rule, on the line an operator actually reads. The ledger has
always shown `—` for a route that reports no dollars, and the review
half of this line has said `$?` since step 9 — but the worker half
used `unwrap_or(0.0)`, so a codex-implemented task printed
`gpt-5.6-sol $0.0000` above a ledger row reading `—`. One run, two
answers, and the wrong one is the one that looks precise.

## `task.attempts = vec![AttemptRecord {`

The attempt that actually ran, as a route reporting no dollars
records it. Without this the task has no attempts at all, which is a
different thing entirely — nothing ran, so nothing is missing, and
the ledger correctly prints `—` rather than a floor.

## `assert!(`

And the same rule one level up. `total_cost_usd` is an `f64`, so it
cannot distinguish a zero sum from an unreported one — the floor has
to be carried beside it. Measured on run 01KZRTZ9ZKKF1YS7MVT4350X7M,
where a codex-implemented task made `total $0.1561` read as complete
while the worker's real spend was unknown.

## `let row = ledger`

Here every attempt was unpriced, so the worker column is `—`, which
already says "unreported" — `partial` leaves it alone rather than
decorating it into `—?`.

## `let mut mixed = priced_and_unpriced_attempts();`

The `?` belongs on a figure that exists but is short: two attempts,
one priced and one not. That is what a resumed run looks like after
the engine was killed inside the first attempt, and what a mixed
ladder looks like when one rung reports and another does not.

## `let mut priced = report;`

And a route that does report keeps its figure.

## `let text = r#"{`

`report.json` is a projection for whoever reads the run afterwards,
and `TaskRunStatus` is `pub` and `Deserialize` because that reader
may be someone else's program. Every variant added to a serde-tagged
enum with no fallback is a hard `unknown variant` error in every
consumer built against an older version — one unreadable status makes
the entire report unreadable.

`running`, `Queued` and `Running` did that to anything compiled
against 0.0.1, and that break is published and cannot be taken back.
This is so the next variant is not another one.

## `assert!(`

And everything the reader *can* understand still arrives intact.

## `let task = Task {`

`Running` says of itself that only a live `status` produces it, and
the arm that built it consulted `in_flight` alone — while the arm
directly below, for `Queued`, guards on `running`. What actually held
the promise was a guarantee made one function away: `settle` turns
every `Pending` into `Skipped` before `task_report` sees it when the
run has ended, so the only way in is `Deferred`, which is recorded
after an attempt finishes and therefore never has anything in flight.

Unreachable is not the same as impossible, and the distance between
the promise and the code keeping it is the whole hazard: a dangling
`in_flight` is what any error out of `run_attempt` leaves behind, and
`drain_and_report` writes a partial `report.json` on exactly that
path. One reordering away, that file reads `t1: running now — attempt
2 on mid` beside a top-level `"running": false`, and outlives the
process that wrote it. So the invariant is stated where it is relied
upon.

## `let (_tree, repo) = temp_engine_repo("resumewindow");`

`resume` takes the run's lock and then does a dozen git subprocesses
— branch checks, a switch, a discard — before it writes
`run_resumed`. Deriving liveness from the lock alone made that whole
window read as a live run: `status` printed `run in progress: N
task(s) committed so far` and returned early, dropping the stop
reason, the parked list, and the `resume --budget` line an operator
at a budget stop is running `status` to find.

The lock answers who has claimed the run. Whether the run still has
anywhere to go is a question only its log answers.

## `let (_tree, repo) = temp_engine_repo("budgetjam");`

Handing back a clean tree was added *before* the ceiling was
recorded, with a `?` on it. So a `git reset --hard` that failed for
any of the ordinary reasons — a locked index, a read-only path, a
hook that exits non-zero — took the whole budget stop with it: no
`budget_exceeded` event, `budget_stop` left `None`, exit 1 with a git
error where CI was gating on exit 3, and a `resume --budget` with no
stop to get past. The tidying is a courtesy; the ceiling is the run's
account of why it stopped.

The fake reviewer plants a stale lock only after its exact candidate
worktree exists. This reaches the budget-stop cleanup without using
ordinary gate residue to pierce the new workspace isolation.

## `assert!(`

And it says so rather than leaving the operator to find the mess.

## `let (_tree, repo) = temp_engine_repo("budgetdecline");`

A decline routes through `fail_task`, which sets `halted_at`, and
halted outranks budget in `outcome()`. A decline sitting on disk when
the ceiling hits would have relabelled the stop as a task failure —
exit 1 where CI was gating on exit 3 to raise the ceiling.

## `fn a_frontier_escalation_over_the_threshold_parks_for_approval_then_runs_it() {`

Since J16 it also asserts that the approval question names its task (`t1`)
and that its context opens with the task line, `` Task `t1` — One ``: the
question is built by `build_question` and described from the plan task it
names, and must read as it did when `build_spend_approval` indexed the plan
itself.

## `fn a_rate_limit_marks_its_pool_exhausted_and_a_recovery_retires_the_signal() {` › `assert_eq!(signals[0].reset_at, None, "{signals:?}");`

The worker's rate-limit text here is the default "5-hour limit reached", which names no reset, so
the recorded `pool_exhausted` carries none. Reading a reset out of text that has none would put an
invented time into the log as the subscription's own.

## `fn a_rate_limit_that_names_its_reset_records_it_on_pool_exhausted() {`

The same run as the test above, with one difference: the worker's rate-limit text is the message
Claude Code printed on 2026-11-11, `You've hit your 5-hour limit · resets 5:30am (UTC)`, set through
`worker_rate_limit`. The one `pool_exhausted` event carries that text in its detail and a `reset_at`
in the fixed-width RFC 3339 form ending `T05:30:00Z`. The test reads the clock before and after the
run, and the reset falls in that window extended by twelve hours either side: the 05:30 UTC nearest
the moment the signal was recorded, which may be the one just past, by string comparison, which the
fixed width makes legitimate. The
estimate folded from the log up to the signal reports the pool exhausted with that same reset time:
`attempt_finished` comes first and marks the pool with no reset, and `pool_exhausted` after it
supplies the time. The run still completes, as the test above does, so the reset changes what is
recorded and nothing about how the attempt is deferred or retried.

## `fn a_limit_on_stderr_beside_an_unrelated_result_records_its_reset_on_pool_exhausted() {`

`PR380-RESET-SIGNAL-CHANNEL-DROPPED`, end to end. With `worker_limit_on_stderr` set, the fake's
rate-limited worker runs as Claude Code did: it prints `CLAUDE_REQUEST_FAILED`, a JSON result
reading `request failed`, on stdout, prints `You've hit your 5-hour limit · resets 5:30am (UTC)` on
stderr, and exits 1. The fake hands that output to the real `ClaudeCodeAdapter::parse` rather than
building the outcome itself, so the classification is `claude::parse_output`'s own. The
rate-limited `attempt_finished` still reads `pool rate-limited: request failed`, and the one
`pool_exhausted` carries that same reason as its detail, as before. Its `reset_at` ends
`T05:30:00Z`. Before the failure carried the stderr, the reset was read from the reason alone and
was `None`.

## `fn a_limit_on_stderr_outranks_an_unrelated_reset_in_the_result_on_pool_exhausted() {`

`PR380-UNRELATED-REASON-RESET-PREEMPTS-LIMIT`. The run above, with the JSON result
`CLAUDE_UNRELATED_RESET` (`MCP cache resets 9am (UTC)`) through `worker_limit_result`. The rate-limited
`attempt_finished` still reads that reason and the one `pool_exhausted` carries it as its detail,
but its `reset_at` ends `T05:30:00Z`: the limit on stderr is read before the reason. When the
reason was read first, its lone `resets` won and the pool was recorded as back at 09:00.

The test first checked that the reason, `pool rate-limited: MCP cache resets 9am (UTC)`, parsed
on its own to a `T09:00:00Z` reset, so that reading it first was what would record the wrong time.
Since `PR380-UNRELATED-LONE-RESET-ATTRIBUTED` the parser reads no reset that no limit gives, so the
check is now that the reason alone gives `None`: the reason cannot record the wrong time in either
order, and the stderr's 05:30 is still what the test requires.

## `fn an_unreadable_reset_on_stderr_is_not_replaced_by_the_results_on_pool_exhausted() {`

The regression review's reset finding, end to end. Stderr carries `You've hit your 5-hour limit -
resets 5:30am (Europe/London)`, a limit whose reset is in a zone the parser refuses, and the JSON
result is, in one run, `CLAUDE_UNRELATED_RESET` (`MCP cache resets 9am (UTC)`) and, in a second,
`You've hit your weekly limit - resets 9am (UTC)`, a limit with a UTC reset of its own. Each run's
rate-limited `attempt_finished` reads `pool rate-limited: ` and its result, the one `pool_exhausted`
carries that reason as its detail, and its `reset_at` is `None`. The stderr names a limit, so it
alone is read, and its reset cannot be. Before, the coordinator's `find_map` went on to the reason
when the stderr gave no reset, and recorded 09:00 from either result; the first run is also held
by the parser, which no longer reads a reset no limit gives, and only the second fails if the
coordinator reads the reason after an unreadable stderr reset.

## `fn a_reviewers_limit_on_stderr_beside_an_unrelated_result_records_its_reset_on_pool_exhausted() {`

`PR380-REVIEWER-STDERR-RESET-DROPPED`, the reviewer's side of the test two above. The worker edits,
and the Claude Code reviewer is refused once and then passes. With `review_limit_on_stderr` set,
the refused review prints `CLAUDE_REVIEW_REQUEST_FAILED`, a JSON result reading `request failed`,
on stdout, `You've hit your 5-hour limit - resets 5:30am (UTC)` on stderr, and exits 1, and the
real `ClaudeCodeAdapter::parse` classifies it. The first attempt's failure is `RateLimited`, from
the reviewer, with the reason `reviewer unavailable: request failed`, and the one `pool_exhausted`
names `claude-max` and `claude-code`, carries that reason as its detail, and has a `reset_at` ending
`T05:30:00Z`. Before `run_review` kept the stderr, the reviewer's result held only its status and
detail, and the reset was `None`.

## `fn a_reviewers_limit_in_the_result_past_the_reasons_head_records_its_reset_on_pool_exhausted() {`

The test above with the limit moved from stderr into the result, past the 400 bytes of it a
reviewer's reason keeps. Through `review_limit_result` the refused review prints a JSON result
reading `MCP cache resets 9am (UTC). `, five lines of MCP diagnostics, then `You've hit your 5-hour
limit - resets 5:30am (UTC)`; its stderr, through `review_limit_on_stderr`, is `request failed`,
which signals nothing. The test first checks that the limit starts past byte 400 of the result and
that the reason built from the result's 400-byte head gives no reset, so reading only the reason
records none. The first attempt's failure is `RateLimited`, from the reviewer, with exactly that
reason, so what is displayed has not changed; the one `pool_exhausted` names `claude-max` and
`claude-code`, carries that reason's own head as its detail, and has a `reset_at` ending
`T05:30:00Z`. Before `run_review` kept the whole result as the limit's text, `limit_message` was
`None`, only the cut reason was read, and the reset was 09:00 and then, once the parser stopped
reading a reset no limit gives, `None`.

## `fn a_reviewers_quota_limit_in_the_result_outranks_a_generic_limit_on_stderr_on_pool_exhausted() {`

`PR380-GENERIC-STDERR-HIDES-REVIEW-RESET`, the test above with a stderr that does signal a limit
but names no quota: `HTTP 429: rate limit`. The test first checks that `looks_rate_limited`
accepts that stderr, that `names_a_quota_limit` refuses it, and that the reason built from the
result's 400-byte head names no quota limit either, so only the whole result can give the reset.
The first attempt's failure is `RateLimited`, from the reviewer, with that reason; the one
`pool_exhausted` names `claude-max` and `claude-code`, carries the reason's head as its detail,
and has a `reset_at` ending `T05:30:00Z`. Before `run_review` preferred the text naming a quota
limit, the generic stderr was the first `looks_rate_limited` accepted, so it became the
`limit_message`, and the coordinator, finding no text naming a quota limit, recorded `None`.

## `fn a_rate_limited_failure_carries_a_limit_from_stderr_without_changing_its_reason() {`

The same defect one level down, at `evaluate_outcome`. The output `ClaudeCodeAdapter::parse` reads
as `RateLimited` from its stderr becomes a failure whose kind, reason (`pool rate-limited: request
failed`) and feedback are what they were, and whose `limit_message` is that stderr. Then a
rate-limited outcome whose stderr signals nothing: the reason is the outcome's detail, as before,
and no `limit_message` is carried, because the stderr did not classify the attempt.

## `fn deferral_rate_limit(now: u64) -> (String, String) {`

Claude Code's weekly-limit text built from a clock reading: it names, by date and time, the minute
three hours after `now`, and the second value is that reset in the log's fixed-width RFC 3339 form.
Like Claude Code it leaves the minutes out on the hour, so built at 14:00 on 2026-11-14 it is the
fixed `You've hit your weekly limit · resets Nov 14, 5pm (UTC)` the deferral tests used to carry.
`MONTH_NAMES` spells the month as Claude Code does; the parser's own list is private to
`agent::adapter`.

The engine reads a reset against the real clock on every refusal, and a dated reset resolves to
the occurrence of its month and day nearest that read. A date three hours after the run's own clock
is the nearest occurrence of itself for about half a year either side, so every read during the run
names the same reset, whatever the date and time it runs. The fixed text did not: its nearest Nov 14
moves to the next year at 2027-05-16T05:00:00Z, halfway between two of them.

The tests set the text through `worker_rate_limit`, which holds a `&'static str`, so they leak it
with `String::leak`: a few dozen bytes for each test that runs.

## `fn the_deferral_rate_limit_text_names_one_reset_at_any_time_of_day() {`

The two tests below build their text with `deferral_rate_limit` from the clock as they start, and
the engine reads it against the real clock once per refusal, so the text must name one reset
whenever a run reads it. The test first pins the text's form: built at 2026-11-14T14:00:00Z it is
exactly the fixed text it replaced, with `2026-11-14T17:00:00Z` as its reset. Then, for each of five
build instants, it reads the text at every second of the day that follows and requires the reset
`deferral_rate_limit` returned each time. The instants are 2026-11-11T00:00:00Z, the day Claude
Code printed the text; 2027-05-16T04:59:59Z and 05:00:00Z, either side of the instant the fixed `Nov
14, 5pm` text moved from naming 2026-11-14T17:00:00Z to naming 2027-11-14T17:00:00Z;
2026-12-31T21:30:00Z, whose reset is `Jan 1, 12:30am` in the next year; and 2028-02-29T09:00:00Z,
whose reset is `Feb 29, 12pm` on a date most years lack. The time-only `resets 5:30am (UTC)` that
came before the fixed text changed reset at 17:30:00 each day, where the next morning's 05:30 took
over.

## `fn a_deferral_rate_limit_keeps_the_reset_pool_exhausted_recorded() {`

The same run as `a_rate_limit_that_names_its_reset_records_it_on_pool_exhausted`, rate limited
twice before it succeeds: `[RateLimited, RateLimited, EditFile]`, with a text that names the reset. The engine writes one `pool_exhausted` for the
outage, after the first rate-limited `attempt_finished` and before the second; the second is the
first deferral, and it writes no signal of its own. The text is `deferral_rate_limit`'s, built from
the clock as the test starts: a weekly limit that resets, by date, three hours later. It is dated
and built from the run's own clock because the engine reads the reset again on each refusal
against the real clock, and the two reads must name the same reset. A time-only `resets 5:30am
(UTC)` resolves to the nearest 05:30, and the later one wins a tie, so a run whose first refusal
was read at 17:29:59 and whose retry was read at 17:30:00 named two different resets and rightly
wrote a second signal. The fixed `resets Nov 14, 5pm (UTC)` that replaced it did the same either
side of 2027-05-16T05:00:00Z, where its nearest Nov 14 moves to the next year. A date three hours
ahead of the run gives both reads the same reset on any day. The estimate folded from the log through that second
`attempt_finished` still reports the pool exhausted with the reset the signal recorded.
Before `retire_signals` kept a known reset, that settlement marked the pool again with none, so
`upstroke validate` said the signal carried no reset time and a resume's `capacity_snapshot` wrote
`reset_at: null` beside an open signal that named one. The estimate over the whole log no longer
reads the pool as exhausted: the attempt that succeeds still retires it.

## `fn a_different_later_reset_writes_one_more_pool_exhausted() {`

The same run as the test above, but the two refusals say different things, through
`worker_rate_limits`: first `You've hit your 5-hour limit · resets 5:30am (UTC)`, then `You've hit
your weekly limit · resets Nov 14, 5pm (UTC)` — a pool refused for its weekly cap on the deferral
after its five-hour cap cleared. Two `pool_exhausted` events, one after each rate-limited
`attempt_finished`: the first carries the `T05:30:00Z` reset, the second the `-11-14T17:00:00Z` one
and the weekly text in its detail. The estimate folded through the second signal reports the pool
exhausted until the weekly reset. Before the engine remembered each pool's recorded reset, the
second refusal wrote nothing, and the estimate kept a 05:30 that had already passed. The test above,
where the same reset is named twice, and `one_outage_records_one_signal_however_many_deferrals_it_causes`,
where no reset is named, still write one signal each.

## `fn a_rate_limit_after_a_success_retired_its_pool_writes_pool_exhausted_again() {`

Two tasks on one pool, each refused once with the same text and then served: `[RateLimited,
EditFile, RateLimited, EditFile]`. The success between the refusals retires the first signal, so
the second refusal writes a second `pool_exhausted` naming the same reset, and the estimate folded
through it reports that reset rather than "unknown". `capacity.md` and `coordinator.md` give the
defect. The text is `deferral_rate_limit`'s, built from the clock as the test starts, and the first
signal's reset is the one it names. The test needs both reads to name one reset: two different
resets write two signals with or without the fix, so only the same reset read twice tells them
apart. The fixed `resets Nov 14, 5pm (UTC)` this test used to carry named the next year's Nov 14 on
a read at or after 2027-05-16T05:00:00Z, so a run whose two refusals fell either side of that
instant recorded two different resets and failed the test's equality.

## `fn another_agents_rate_limit_naming_a_reset_records_none_on_pool_exhausted() {`

The reset is read only from Claude Code's text. The small tier is pinned to `copilot`, the pools
file names only `copilot-window`, and the Copilot fake refuses once with `premium request allowance
exhausted; you've hit your weekly limit · resets Dec 1, 12am (UTC)` before it edits; Claude Code
reviews. The test first checks that `agent::reset_at_from_limit_message` does parse that text on
its own, so the only thing that can keep the reset out is the engine's check of the agent. The text
was `premium request allowance exhausted; resets Dec 1, 12am (UTC)` until the parser stopped
reading a reset no limit gives; that text no longer parses, so it could no longer tell the agent
check from the parser, and the weekly limit was put before its reset. The one `pool_exhausted` names
`copilot-window` and `copilot` and carries the text in its detail, and its `reset_at` is `None`.
Before the engine compared the agent with `claude::ADAPTER_ID`, it recorded the nearest 1 December
at 00:00 UTC there: a reset read from another vendor's words on the strength of Claude Code's.
