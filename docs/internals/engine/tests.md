# `src/engine/tests.rs`

Extended notes for [`src/engine/tests.rs`](../../../src/engine/tests.rs).

## `#![allow(clippy::disallowed_methods, clippy::disallowed_types)]`

LEGACY-EFFECT: this module is in the **frozen legacy section** of
`effects/allowlist.toml`, which carries its justification and the condition
under which the section shrinks. `decisions.effect_site_inventory.mechanism` (2).

## `EditFile,`

Simulates an agent that edits the workspace and succeeds.

## `EditTest,`

Simulates an agent that writes real test code.

## `LargeEdit,`

Produces a complete diff larger than the review input boundary.

## `OpaqueEdit,`

Produces a binary patch whose changed bytes cannot be semantically
reviewed from the unified diff.

## `IgnoredGateInput,`

Adds an ignored, uncommitted input that a gate could observe in the
worker tree but that is absent from the staged review candidate.

## `FrozenCandidate,`

Lets the test mutate the authoritative index after capture and
records which immutable tree the reviewer actually received.

## `JamCleanupAfterReview,`

After the reviewer workspace exists, plants a stale lock in the
common git directory so the later authoritative cleanup fails.

## `LargeEditQuestionWriteFailure,`

Produces the same oversized diff, then makes the question payload
directory unwritable so parking preparation fails deterministically.

## `NoEdit,`

Simulates a lying agent: success report, no changes.

## `SpawnError,`

Command construction succeeds, but the worker executable cannot be
spawned — the shape of an agent CLI removed, renamed, or self-updated
out from under a run that has already passed pre-flight.

## `Error,`

Simulates an agent-side failure.

## `RateLimited,`

Simulates the pool being exhausted.

## `AskQuestion,`

Edits, then stops and asks the operator a question (§12).

## `Exit,`

Kills the whole process partway through the attempt, leaving the
on-disk shape a `kill -9` or a power loss leaves: a dirty working
tree and an `attempt_started` with no `attempt_finished`.

## `const CRASH_EXIT_CODE: i32 = 42;`

Distinctive so the parent can tell a deliberate death from a panic,
which would also exit non-zero.

## `Unparseable,`

Prose with no verdict block: drives the re-ask path.

## `RateLimited,`

The judge itself could not run.

## `SpawnError,`

Command construction succeeds, but the reviewer executable cannot
be spawned.

## `NeedsHuman,`

§12: the reviewer declines to judge and asks for a person.

## `struct FakeAdapter {`

Scripted stand-in for a real CLI. `build` performs the "agent edit"
directly (test-only shortcut) and returns a trivial command; `parse`
reports the scripted outcome. Read-only profiles are review
invocations and answer with a verdict, exercising the real
command → stdout → parse → verdict path.

Both scripts are consumed per invocation and the final entry repeats,
so a one-element script behaves exactly like the fixed adapter did.

## `id: &'static str,`

Which agent this stands in for. Cross-vendor tests (§11.3) need two
ids, because "a different model family" is unreachable otherwise.

## `probe_error: Option<&'static str>,`

Simulates a CLI that is installed but broken, for the pre-flight
probe classes: required agents refuse the run, the opportunistic
cross-family one only warns.

## `reports_cost: bool,`

Whether this route reports spend. Copilot's does not.

## `worker_rate_limit: &'static str,`

The detail a worker's `Effect::RateLimited` reports, which the engine
carries into the failure's reason (`pool rate-limited: …`) and so into the
text `pool_exhausted` reads its reset from. Both constructors set it to
[`FIVE_HOUR_LIMIT_REACHED`], the text every test saw before the field
existed, so only a test that assigns it sees anything else. The reviewer's
rate limit keeps its own literal.

## `worker_rate_limits: Vec<&'static str>,`

The same detail scripted per worker call, indexed like `effects`: a run
whose refusals say different things — the five-hour limit, then the
weekly one — sets this. Past the script's end the last entry repeats,
and an empty script (both constructors' default) leaves every refusal
saying `worker_rate_limit`.

## `worker_limit_on_stderr: Option<&'static str>,`

A limit Claude Code printed on stderr beside an unrelated JSON result. When
set, a worker's `Effect::RateLimited` runs a real process that prints
`worker_limit_result` on stdout and this text on stderr and exits 1, and
`parse` hands that output to the real `ClaudeCodeAdapter::parse`, so
`claude::parse_output` decides the outcome and its detail. Both constructors
leave it `None`, and then the scripted detail above is used, as before.

## `worker_limit_result: &'static str,`

The JSON result that stdout carries beside `worker_limit_on_stderr`, and
only then. Both constructors set it to [`CLAUDE_REQUEST_FAILED`], whose
result names no reset; [`CLAUDE_UNRELATED_RESET`] is the one whose result
names a reset that is not the limit's.

## `review_limit_on_stderr: Option<&'static str>,`

`worker_limit_on_stderr` for a reviewer. When set, a review whose script says
`ReviewBehavior::RateLimited` runs a real process that prints
`review_limit_result` on stdout and this text on stderr and exits 1, and
`parse` hands that output to the real `ClaudeCodeAdapter::parse`. Both
constructors leave it `None`, and then a rate-limited review reports the
scripted `5-hour limit reached` detail, as before. A text that signals no
limit makes a review whose result alone carries it.

## `review_limit_result: &'static str,`

`worker_limit_result` for a reviewer: the JSON result stdout carries beside
`review_limit_on_stderr`, and only then. Both constructors set it to
[`CLAUDE_REVIEW_REQUEST_FAILED`]; a result set here must hold
[`REVIEW_MARKER`] too, or `parse` does not know it for a review's output.

## `const FIVE_HOUR_LIMIT_REACHED: &str = "5-hour limit reached";`

The older Claude Code wording, which names no reset.

## `const CLAUDE_REQUEST_FAILED: &str =`

A Claude Code JSON result that failed for a reason the result does not
name: `request failed`, with `is_error` set. Nothing in it signals a rate
limit, so any limit has to come from stderr.

## `const CLAUDE_REVIEW_REQUEST_FAILED: &str = r#"{"type":"result","is_error":true,"result":"request failed","session_id":"fake-review-session","fake":"UPSTROKE-FAKE-REVIEW"}"#;`

[`CLAUDE_REQUEST_FAILED`] for a reviewer. The extra `fake` field holds
[`REVIEW_MARKER`], which is how `parse` knows a review's output; the real
parser ignores a field it does not read.

## `const REVIEW_MARKER: &str = "UPSTROKE-FAKE-REVIEW";`

Marker the fake's review command prints so `parse` can tell a review
invocation from an implementation one.

## `fn copilot(reviews: Vec<ReviewBehavior>) -> Self {`

The second vendor. It only ever reviews in these tests, so it needs
no effects script.

## `fn unpriced(mut self) -> Self {`

Stands in for the Copilot route, which has no JSON envelope and so
reports no spend at all (§13).

## `fn reviews_run(&self) -> usize {`

How many review invocations this adapter was asked for.

## `fn cross_vendor(`

A machine with both CLIs installed: claude-code implements and gives the
acceptance verdict, copilot gives the §11.3 second opinion. Each adapter
keeps its own review script and counter, so a test can say what each
vendor answered and check which of them was asked at all.

## `fn scripted<T: Copy>(script: &[T], index: usize, fallback: T) -> T {`

The last scripted entry repeats forever.

## `return Ok(shell_spec(&format!("echo {REVIEW_MARKER}")));`

No `current_dir`: the runner puts the process in
`RunnerRequest.workspace`, which is `run.workspace`.

## `let _ = fs::write(`

Half-finished edits first, then die without
unwinding — no destructors, no flush of anything the
engine has not already synced. That is what makes
this a faithful stand-in for a kill rather than a
tidy shutdown, and it happens at a deterministic
point instead of racing a signal.

## `return Ok(CommandSpec::new(`

Return before editing anything: this attempt never gets to
run, so it must not leave the workspace looking as though it
did.

## `fn materialize_permissions(`

Delegate to the real generator so the engine's permission wiring is
exercised, not stubbed out.

## `let index = self`

`build` already consumed this invocation's slot.

## `Effect::EditFile`

`Exit` never reaches here — `build` ends the process.

## `| Effect::SpawnError`

`SpawnError` never reaches here either: the runner fails to
spawn it, so nothing is parsed.

## `copilot: Option<FakeAdapter>,`

`None` is the single-vendor machine — which is also the shape that
makes a cross-family reviewer unresolvable.

## `struct ScriptedAnswers {`

Answers handed out in order; anything past the script is unanswered,
which is exactly how a detached terminal behaves.

## `fn shell_spec(script: &str) -> CommandSpec {`

Shared with the production path so tests exercise the same shell
invocation (including its Windows quoting) rather than a parallel one.

## `fn limit_on_stderr_spec(result: &str, limit: &str) -> CommandSpec {`

The process `worker_limit_on_stderr` and `review_limit_on_stderr` both run:
a JSON result on stdout, a limit (or, for a limit the result carries, other
text) on stderr, a failing exit. One spelling for
both, so the worker's and the reviewer's limit reach `claude::parse_output`
the same way, and the comment in it says why the spelling is the one both
shells share.

## `fn temp_engine_repo(tag: &str) -> (ScratchTree, PathBuf) {` › `git_in(&dir, &["config", "maintenance.auto", "false"]);`

The fixture's repository runs no automatic `git maintenance` (K7). Git 2.47 and
later detach `git maintenance run --auto` after a commit, and it holds
`.git/objects/maintenance.lock` while it decides whether to work; in a test's
tree that is a process the test did not start, writing beside a test that may be
reading. CI's macOS leg ran git 2.56.0 and the J14 test's walk of this tree
panicked on an entry it had listed and could no longer find. Set before the
seed commit, so no commit in the fixture's repository, the engine's included,
starts one. Nothing in the engine depends on automatic maintenance.

## `fn seed(repo: &Path, plan: &str, config: Option<&str>) {`

Replace the plan and config, then commit so the tree is clean.

## `opts.defer_backoff = Duration::ZERO;`

Tests must never actually wait — not out a rate limit, and not at a
hard block either. The test harness has no terminal, so an
interactive mode resolves to the waiting answer channel; without a
zero budget every parking test would sit out the real one.

## `fn no_pools() -> PathBuf {`

An explicit pools path with no pools in it.

A real, empty file rather than an absent one: an explicit `--pools` that
does not exist is a hard error now, and `None` would reach for the
operator's real `~/.upstroke/pools.toml` — which no test may touch.
An empty pools file, created once for the whole test process.

Every test routes through here, and this used to *rewrite* the file on
each call — one shared path truncated and rewritten while other threads
were reading it. The content is the same for every caller, so there is
nothing to rewrite: build it once and hand back the path.

## `fn private_root_for(repo: &Path) -> PathBuf {`

A scratch stand-in for `~/.upstroke`, so tests never touch the real one.

A *sibling* of the repo, never a directory inside it. That is not
tidiness: §14's rollback is `git clean -fd`, which deletes untracked
directories — a private root inside the workspace would have its
transcripts and verdicts destroyed by the first failed attempt. The
same reasoning is why production puts it under the user's home.

## `fn resume_options(repo: &Path, run_id: &str) -> ResumeOptions {`

Resume options matching [`options`], for the same reasons.

## `fn paths_of(repo: &Path, run_id: &str) -> RunPaths {`

The paths a test's run wrote to.

## `const FRONTIER_AUTH_PLAN: &str = "## Rotate the signing key\n\`

---- step 9: cross-vendor review (§11.3) -----------------------------
A plan whose one task runs at frontier and touches `src/auth/**`, so
both step-9 mechanisms are in play: its implementer binds to the same
model as the reviewer, and its paths can match a `second_opinion`
override.

## `const FRONTIER_ONLY_CONFIG: &str =`

Same task, no override — the implicit anti-self-review path.

## `fn worktree_lock_path(repo: &Path) -> PathBuf {`

Where this repository's worktree lease file would be, held or not.

## `fn replay_of(repo: &Path, run_id: &str) -> crate::status::RunStatus {`

---- step 8: the event log is the state ------------------------------
Fold a run's log the way `status` and `resume` do.

## `fn assert_live_equals_replay(repo: &Path, live: &RunState, report: &RunReport) {`

The property the whole design rests on: a live run and a replay of its
own log are the same computation, not two that happen to agree.

Asserted on `RunState` rather than on the report, because the report is
a lossy projection — it drops `feedback`, `resume_next`, `session`, and
the rung a task is standing on, which are exactly the fields a resume
depends on being right.

## `let strip = |report: &RunReport| {`

Warnings are the one field deliberately excluded. They are
diagnostics of the *process* — what this invocation noticed about a
missing notifier or a discarded working tree — not facts about the
run, so a later reader legitimately has different ones. Anything
that genuinely belongs to the run is an event instead (a discarded
tree, for instance, rides on `run_resumed`).

## `fn task_report_costing(worker: Option<f64>, review: Option<f64>) -> TaskReport {`

A report carrying nothing but the two cost columns.

## `#[test]`

Spawned by `killing_a_run_mid_attempt_leaves_a_resumable_record`.
Ends its own process on purpose, which is why it must never run as part
of the ordinary suite.

## `let source = source(`

t1 commits; the process dies inside t2's first attempt.

## `std::process::exit(0);`

Only reachable if the adapter never got a second invocation, which
would mean this test is not exercising what it claims to.

## `const V1_OBJECT_GRAPH_GATES: &str = "[[gates]]\nname = \"snapshot-bytes\"\n\`

Two gates. Each one's verdict is one half of the graph question and nothing
else. Both read the committed `probe.txt`, whose blob `refs/replace/*` sends
to a blob reading `replacing`.

`snapshot-bytes` reads the file the snapshot holds with `git grep --no-index`,
which reads the working tree and no object, so it passes only if the v0.1
workspace materialised the recorded blob. `object-graph` reads the same path
through Git at `HEAD`, so it passes only if the runner's child refuses the
replacement. Measured on git 2.43 with plain `git`, over one checkout written
with replacements honoured and one written with them refused.
`snapshot-bytes` exits 1 over the first and 0 over the second. `object-graph`
exits 1 in a child that honours the replacement and 0 in one that refuses
it, over either checkout.

The replacement is on a blob, not a tree or a commit. The candidate tree is
the agent's, not the fixture's, so the one object a fixture can replace in
advance and know it reaches every snapshot of the run is a blob in the tree
the run starts from. That is also why neither gate is `git diff --exit-code
HEAD`. A snapshot's index records the blob's own id, so comparing ids passes
over replaced bytes without reading them: measured, exit 0 under both graphs
over the checkout written with replacements refused. Neither gate uses a
shell builtin or platform-specific quoting. Each is one `git` command, the
same under `sh -c` and `cmd /C`.

## `fn v1_object_graph_helper()` › `crate::workspace_manager::fixture::REPLACEMENT_WITNESS`

The marker the helper refuses to run without, for the reason every
`#[ignore]` helper in this crate has one: a run with `--include-ignored`
would otherwise execute its body in a process whose environment nobody
prepared, which is the one environment this witness must never be measured
in. It is
[`fixture`](../../../src/workspace_manager/fixture.rs)'s rather than this
module's because every replacement witness in the crate now shares one, and
one marker beside one door is the whole of that (PR #271, round 4).

## `fn replaced_probe_repo(tag: &str, plan: &str, config: &str) -> (ScratchTree, PathBuf) {`

An engine repository whose committed `probe.txt` carries a replacement the
v0.1 workspace must refuse.

`probe.txt` is written before `seed` commits. So it is in the tree the run
starts from and in every candidate tree the run builds, since the agent's
edit touches another path. The replacing blob is written with `git
hash-object -w` from a file that is then removed, so the worktree is clean
when the run starts, and the replacement ref keeps that blob reachable.
Before it returns, the fixture states its premise with a Git child of its
own that honours replacements: `git show HEAD:probe.txt` reads `replacing`.
Without that, neither gate would be measuring anything.

## `#[test]`

The v0.1 conductor runs and resumes on the graph its own workspace wrote. Since
`LEGACY-WORKSPACE-READS-REPLACEMENT-OBJECTS` closed, that is the recorded
graph. The test first came from PR #271, round 2's fix-check finding.

Until that finding closed, this test pinned the opposite. Its one gate, `git
diff --exit-code probe-recorded probe-replacing`, passed only when the child
read the replaced graph, so it held the conductor to `ObjectGraph::AsReplaced`.
Its replacement never reached a gate snapshot either: the two probe commits
were tags on a deleted branch, in no tree the run read, so it measured the
runner and never the producer. It pinned the old design, so it was inverted
rather than deleted, and the replacement moved into the tree every snapshot
of the run is made from.

`src/gates.rs`'s `a_v1_gate_judges_the_tree_its_own_workspace_materialised`
drives `HostRunner::for_legacy_workspace()`, and the environment test pins
what that constructor reads. Neither reaches the two sites that call it, and
`production_reaches_a_spawn_through_one_host_runner_per_run` accepts either
constructor by design. So this is the test that notices a site building some
other runner. That gap is why it exists: at head `aa2728d`, before it did,
reverting the two calls to `HostRunner::new()` left the gates witness, the
environment test and that census all green. The calls were then in
`src/engine/mod.rs`; since 2026-09-20 they are in `src/engine/coordinator.rs`
and `src/engine/resume.rs`.

It drives `engine::run_with` and `engine::resume_with`, the production
facades one call above each of `run_harness` and `resume_harness`, so each
site is guarded on its own. Through them it drives the production producer
(`gate_snapshot_for_candidate_in_store`) and the production runner. Measured
by mutation at this head:

| mutation | where the test failed |
|---|---|
| `for_legacy_workspace` reading `AsReplaced` | the run leg: every attempt `GateFailed` on `object-graph`, `snapshot-bytes` passing, the run parked |
| `run_harness` building a runner reading `AsReplaced` | the run leg, the same way |
| `resume_harness` building a runner reading `AsReplaced` | the resume leg only: the first run parks as it is meant to, the answer un-parks it, and the resumed attempt parks on `object-graph` |
| `git_command` without the pair | the run leg, before any gate: `verify_gate_worktree` refused the snapshot with ` M probe.txt` |
| both halves reverted, the behaviour of `a3767bcc` | the run leg, as the row above |

The fourth row is the producer's own post-checkout check firing, not
`snapshot-bytes`. The checkout wrote the replacing bytes under an index that
records the recorded blob, and `git status` in the snapshot found the file
modified. So with both halves reverted, a v0.1 run whose candidate tree
carries a replaced blob does not reach its gates at all. `snapshot-bytes`
states the producer's half directly, not through that check.

The resume leg takes `a_parked_run_is_answered_out_of_band_and_resumed`'s
shape — `Effect::NoEdit` parks the task before any gate runs, the answer is
written by the CLI path, and the resumed attempt is the first one to reach
a gate — because a resume of a completed run replays and runs nothing.

## `fn resume_err(repo: &Path, run_id: &str) -> String {`

---- step 8: resume refuses rather than guessing -----------------------

## `const PARKED_RUN_CONFIG: &str = "[interaction]\nmode = \"never\"\n\n\`

The base config every parked-run fixture starts from: one rung, one
attempt, no interaction — so a task that cannot pass parks immediately.

## `fn parked_run(tag: &str) -> (ScratchTree, PathBuf, String) {`

A run that ends parked — the resumable shape every refusal test starts
from, so each one isolates exactly the thing it breaks.

## `fn parked_run_with_config(tag: &str, config: &str) -> (ScratchTree, PathBuf, String) {`

As [`parked_run`], with the config spelled out — for the tests that need
a `[[gates]]` section in the record.

One recipe, not two: the chains check runs before anything gate-related,
so a copy whose `[routing]` line drifted from the original would fail
these tests on "routing has changed" and point at the wrong thing.

## `fn resume_answering(repo: &Path, run_id: &str, effect: Effect) -> RunReport {`

Resume and answer the question the parked task is waiting on, so the
task actually runs again and its gates actually execute.

## `fn rewrite_run_started_as_schema_two(paths: &RunPaths) {`

Rewrite a current start into the shape written immediately before the
complete-review contract: schema 2 and no per-pass timeout field.

## `fn truncate_log_after(paths: &RunPaths, event: &str) {`

Rewind a log through the named event — the shape a process killed
immediately after its durable transition leaves behind.

## `#[derive(Debug, Clone)]`

---------------------------------------------------------------------------
PR4: every process the legacy engine starts goes through the Runner
---------------------------------------------------------------------------
One process the engine asked a runner to execute.

## `stdin: String,`

What the child receives on stdin. The adapter says *whether* a prompt
is delivered this way (`AgentAdapter::stdin_payload`); the spec is what
carries the bytes, and the runner is what writes them.

## `struct RecordingRunner {`

A real [`HostRunner`](../../../src/runner/host.rs) that writes down what
it was asked to run.

It delegates rather than stubs: a recorder that returned canned output
would prove the engine *called* something and nothing about the run still
working. Every assertion below is therefore made about a run that actually
committed its tasks.

## `#[test]`

The engine facade's public surface is the one the packet enumerates — no
wider.

`decisions.phase_zero_modules.visibility`: "pub(super) only where a sibling
or tests reference an item; **no new pub or pub(crate)**; public paths
unchanged", and `modules["src/engine/mod.rs"]` lists the facade item by
item: the five `pub use` groups, `pub fn run/run_with/run_harness`, and
`pub fn resume/resume_with/resume_harness`.

**The six functions are re-exports since 2026-09-20, and the public paths are
the packet's still.** Their definitions moved into `engine::coordinator` and
`engine::resume`, the conductor modules they drive, so that the facade calls
nothing denied and carries no allow (`PR306-FACADE-INLINE-ESCAPE`; the facade's
own notes say why a file a topology module descends from cannot carry one). So
the census reads the facade's `pub use` statements for them: the names
re-exported from the two conductor modules are exactly the six, the facade
declares no `pub fn` of its own, each conductor module's top-level `pub fn`s
are exactly the ones re-exported from it -- so a seventh cannot be defined
beside them and left unexported, waiting for a `pub use` -- and everything
else re-exported is still the packet's five groups, eighteen names.

This slice added `run_harness_on` and `resume_harness_on`, which take the
boundary as a parameter. Inside the crate that is exactly right — it is how
`engine::tests` drives a recording runner. Public, it is a hole in
`invariants[22]` ("schema-1..3 runs are host-only and no run changes its
boundary or image between epochs"): a downstream crate could execute a
legacy run through a Docker or remote `Runner` with no `RunnerPolicy`
recorded and no refusal, and a later ordinary `resume` would move it back
on-host between epochs.

A `pub`/`pub(crate)` widening is invisible to every in-crate test — each
caller compiles either way — so this reads the facade's own text. The
expected sets are transcribed from the packet, not derived from the file.

## `let raw = include_str!("mod.rs");`

**The blanked region, not the raw file.** This read `include_str!` and
counted prose: the doc comment on `pub(crate) mod topology;` explains
that this census forbids `pub mod `, and that sentence *is* a `pub mod `
in the file — so the census failed on its own explanation.
`PR4-CENSUS-COMMENT-ORACLE`, and the same trick would have let any of the
six widenings be smuggled past by writing it in a comment.

## `top_level_public_fns(source),`

Every `pub fn` at the facade's top level, and there are none: the entry points
are defined in the conductor modules and re-exported.

## `let entry_points: BTreeSet<String> = public_facade_entry_points().into_iter().collect();`

The six the packet enumerates, read from the facade's re-exports of the two
conductor modules.

## `for widening in [`

And nothing else is exported by any other route.

## `"pub mod ",`

**`pub mod ` was missing from this list, and it is the route the
schema-4 surface actually took.** `pub mod topology;` stood here for
the whole slice: this census enumerated functions and re-exports, so
a whole subsystem reached the public path without moving any number
it counts. `create_run`, `Started::into_handle`,
`TopologyRun::resumed` and `step` were reachable by any downstream
caller of this library, which could then write schema-4 state that
this build's own recovery refuses — the frontier review of
`75da796`, finding 1. A census that forbids five widenings and not
the sixth is the shape of every fail-open needle this slice found.

## `let reexported: BTreeSet<&str> = facade_reexports(source)`

The re-exports other than the entry points, flattened. `pub use` is the other
way a name reaches the public path, and the packet enumerates these too.

## `"RunOptions",`

options

## `"RunReport",`

report

## `"AdapterSource",`

crate::agent

## `"AttemptRecord",`

crate::events

## `"AttemptFailure",`

crate::ladder

## `let conductors = [`

The two conductor modules the entry points are re-exported from, each with its
boundary-taking seam. The seams exist, as `pub(super)` items of their modules
— which is `engine` and its descendants, the set the facade's private items
were visible to — and are *not* public, by either spelling, and are not
re-exported: this test would pass just as well if they had been deleted, which
is not what it is for.

## `const CONDUCTOR_MODULES: [&str; 2] = ["coordinator", "resume"];`

The modules whose re-exported functions are the engine's write-coordinator
entry points.

## `fn top_level_public_fns(production: &str) -> std::collections::BTreeSet<&str> {`

Every `pub fn` written at the top level of a module's production code.

## `fn facade_reexports(production: &str) -> Vec<(&str, Vec<&str>)> {`

Each `pub use` statement of the facade as the path it re-exports from and the
names it re-exports.

## `fn public_facade_entry_points() -> Vec<String> {`

The six public entry points of the facade, as the facade's own text spells
them: the names it re-exports from the conductor modules.

Read from `mod.rs` rather than written out, so a seventh public entry point
cannot be added without appearing here — and therefore without being
classified by the two tests below, which cross this set against a table of
calls.

## `const PARKING_SETTLEMENT_KILL_CHILD: &str = "engine::tests::parking_settlement_kill_child";`

The first kill child of the question-payload witnesses: the run dies once its parking settlement
is durable.

## `const KILL_CHILD_BOUND: Duration = Duration::from_secs(120);`

The deadline both kill children are given through `workspace_manager::fixture::run_kill_child_within`:
the topology scaffold's `KILL_CHILD_BOUND`, which this module cannot name, at the same 120 seconds. A
child still running at the bound is killed and reaped, and the witness fails naming its tag.

## `const ASKING_PLAN: &str =`

One implementer task, whose worker (`Effect::AskQuestion`) stops and asks.

## `const PARKING_CONFIG: &str = "[interaction]\nmode = \"never\"\n\n\`

Never asks anyone and one attempt per rung, so the worker's question parks the task, and a resume
with no answer parks it again.

## `struct KillOnceTheParkingSettlementIsDurable {`

The legacy log's hooks (`RunOptions::log_hooks`, the seam the legacy append tests use), aborting
the process at the after phase of the append whose line is the `attempt_finished` carrying a
parking decision. The after phase is consulted once the line is written and synced, and the
coordinator's next effect is the payload write (`coordinator.rs`: `materialize_question` follows
the settlement's `emit`, with only the settlement's error check between them), so the process dies
with exactly the durable prefix `RunDir.WriteQuestionPayload`'s before phase has.

## `fn parking_settlement_kill_child() {`

Runs `ASKING_PLAN` under `PARKING_CONFIG` with the hooks above. Reaching the panic means the run
went past its settlement.

## `struct QuestionPayloadKilledAt {`

The run-directory funnels' production adapter with a kill at one phase of
`RunDir.WriteQuestionPayload`, exported before it is handed back.

## `fn payload_phase_named(name: &str) -> crate::topology::effects::HookPhase {`

The phase `UPSTROKE_TEST_KILL_COORDINATE` names, in the parent's `Debug` spelling.

## `fn question_payload_kill_child() {`

Over the run the first child left, writes the parked question's payload through
`rundir::write_question_payload` with the adapter above, and dies at the phase. The arguments are
the ones `interaction::write_question` passes it (the record's filename component, the questions
directory, the record the log's settlement holds): the legacy engine reaches the funnel only
through that wrapper, which passes `NoHooks`, and has no seam to arm it, so this is how a kill at
the payload's own phase is constructed and observed under the production adapter. This child's
record holds the coordinate.

## `fn a_run_killed_once_its_parking_settlement_is_durable(`

Makes the repository, seeds the plan, runs the first kill child, and reads back what it left: the
log ends at the `attempt_finished` with its parking decision, the replayed state holds exactly one
open question, no payload exists, and the run lock went with the process.

The repository and its sibling private root (`private_root_for`) are made inside one
`rundir::scratch_tree` tree, which the helper hands back with them, and each caller holds it for
its whole body. Both are reclaimed when the witness returns and when it unwinds. A reclaim that
fails on the return fails the test naming the root; one that fails while the test unwinds is
reported on stderr beside the failure, without a second panic (`rundir::scratch_tree`'s own tests
witness both). Before #292's review round 6 they were `upstroke-engine-<tag>-<pid>` and its
`-home` in the temporary directory, which nothing removed.

The kill child is given the tree as its temporary directory too (`TMPDIR`, and the `TMP` and
`TEMP` Windows reads), so the pools file `options` makes it through `no_pools`, one tree per
process, lies inside the tree (#292's review round 7: it was
`upstroke-engine-nopools-<pid>` in the temporary directory, which the child's abort left there).

## `fn question_payload(repo: &Path, run_id: &str, record: &QuestionRecord) -> PathBuf {`

Where the question's payload lives.

## `fn resume_options_in(`

`resume_options`, with its empty pools file written in the witness's tree instead of through
`no_pools`, which makes one per test process in the temporary directory and which nothing removes.
The payload witnesses and the ambient-join witness resume through this, so a run of them alone
leaves nothing in the temporary directory (#292's review round 7).

## `fn resume_parked(`

The tabled recovery: `resume_harness_inner`, the legacy resume, whose question rewrite
(`resume.rs`, every record of the replayed state written through `interaction::write_question`) is
the production payload writer's recovery. With no answer the run parks again, the payload holds
exactly the record the settlement recorded, the report names that one question, and the resumed
state and report equal a replay of the log on each of two loads. It resumes with
`resume_options_in` over the witness's tree.

## `struct CreationsRecorded {`

The process funnel's production adapter, recording every `child_created` callback.

## `fn a_resume_whose_ambient_job_join_errs_runs_nothing_and_the_next_resume_converges() {`

Row 145 of Gate 5's audit on Windows, `Process.Spawn`'s `AmbientJobJoined` point in error-return
mode, at the write-command containment boundary with a continuation that can be seen: the legacy
resume facade `resume_contained`, given a run that the first payload child left parked with its
settlement durable. Its containment step is the production `contain_write_command` under the
adapter above with the point armed. The facade refuses with the injected error and the point
fired. Nothing the continuation does happened: no process was created (no `child_created`
callback), the runner ran nothing, the log's bytes are unchanged, the question's payload was not
written, and nothing holds the run lock. The next resume converges as `resume_parked` requires.
Both resumes run with `resume_options_in` over the witness's tree.

## `struct FakeAdapter` › `role_probe: Option<RoleProbe>,`

When set, the worker and reviewer commands are this test binary run as the role
([`RoleProbe`]) rather than `exit 0` and `echo UPSTROKE-FAKE-REVIEW`. Everything
else the fake does is unchanged: its in-process edits, its scripted review
verdicts, its call counts. So a witness gets a real worker and a real reviewer
process, started by the production runner with the environment it composes,
without writing a second fake.

## `const ROLE_PROBE: &str = "UPSTROKE_V1_ROLE_PROBE";`

The directory a role probe reads its `spec` from and writes its record into. The
witness's parent sets it on the child it spawns, so the engine's own
environment carries it and every role the runner starts inherits it: nothing
reaches a role by any route the runner does not compose.

## `struct RoleProbe {`

The worker's and reviewer's command specs, each this test binary at
`v1_role_probe_worker` or `v1_role_probe_reviewer`, with the overlay a witness
hands it; and, optionally, one blob the fake reads with the operator's own Git in
the engine process before each worker starts.

## `impl RoleProbe` › `fn read_as_the_operator_does(&self, workspace: &Path) {`

An external process reading during the run. The fake's `build` runs in the engine
process, whose environment is the operator's, so a `git cat-file -p` there is
what any Git the operator runs beside the run would see. The witnesses assert it
read the replacement every time: the run changed nothing that the operator's Git
reads.

## `fn role_probe_gates() -> String {`

The one gate a role witness configures: this test binary at `v1_role_probe_gate`,
as a command line that `sh -c` and `cmd /C` both run.

## `fn v1_role_probe(role: &str) {`

What one role process reads, as a list of `<role> <check> ok|FAIL <detail>`
lines in its own file under the record directory. Every check is independent,
and none panics: the witness wants the whole record, not the first failure.

- `variable`: no `GIT_NO_REPLACE_OBJECTS` in this process's environment. Its
  presence is the regression this policy replaced.
- `worktree-true` (gates, when the spec says `worktree true`): the role sets
  `core.useReplaceRefs = true` in its own snapshot's worktree configuration
  first, so every read below it has to beat that scope too. It sets it through
  `probe_git_past_another_registrations_write` (proposed and conditional,
  B-W924-R2, its section below): the setting reads every registration of the
  repository first, so beside a sibling run's add in flight it can die on that
  sibling's empty `commondir`.
- `include`: `git config -z --show-origin --get core.useReplaceRefs` answers
  `false` from the include file, in the directory the role was started in. The
  include is present and in force while the role runs, not only when the run
  began.
- `blob`, `tree`, `commit`: each replaced object read by id — `cat-file -p` of
  the blob, `rev-parse <tree>:inner.txt`, `log -1 --format=%s <commit>` — answers
  as the repository records it.
- `candidate`: the worker writes `candidate.txt`, installs a replacement of its
  blob with `git replace -f` and reads the blob back recorded; the replacement is
  installed during the attempt, by a role. Gates and reviewers read the file
  through the snapshot's `HEAD` as recorded, find that replacement in `git
  replace -l`, and see `git diff --exit-code HEAD` pass and `git status
  --porcelain` empty before anything else touches their directory.
- `path-budget`: the length of the longest path Git writes in the repository
  the role makes inside its working directory,
  `fx/.git/refs/replace/<40 hex>.lock`. It fails on Windows at 260 characters,
  `MAX_PATH`. A gate's working directory is the product's snapshot, deep in the
  private root, so this is where a name that grows runs out first, and the record
  says so with the length before Git's "Filename too long" does. On the CI guest,
  until 2026-11-20, the longest was 249 characters, for the shapes witness's glob directory.
- `fixture-inside`, `fixture-outside`: the role makes a repository inside its
  own working directory (named `fx`, for the budget above) and one outside it,
  installs a replacement in each and reads it back replaced. Both keep their
  replacements. So does a repository a role creates elsewhere, except one beside a
  component of the managed repository's path, named like it but for case, in a
  directory an alias makes look as though it folds case
  (`PR326-A-JUNCTION-MAKES-A-CASE-SENSITIVE-DIRECTORY-READ-AS-FOLDING`). Its commit passes
  `-c maintenance.autoDetach=false`, which works around
  `PR326-MACOS-A-DAEMONIZING-DESCENDANT-HANGS-UP-THE-ROLE` on the owner's ruling of
  2026-09-28. Git 2.55.0's commit otherwise detaches its auto-maintenance. It
  forks, the parent exits and the child calls `setsid()`, and on macOS the role
  that ran the commit then died of `SIGHUP` in 15 of 20 probes. The witnesses
  avoid that trigger. The defect is filed with its reproduction, not absent.

A gate exits 1 when any line failed, so a failing gate also fails the attempt;
workers and reviewers exit 0 and leave the verdict to the record.

## `fn v1_role_probe_reviewer() {`

The reviewer's probe prints the fake's review marker after its record is written,
so the fake parses a review and the attempt goes on to the verdict the source
scripts. It writes through `std::io::Write`, because `println!` is a denied macro
in this file.

## `fn install_a_replaced_graph(repo: &Path, probe: &Path) -> (String, String) {`

Commits `blob.txt` and `dir/inner.txt` as `recorded-commit`, then replaces all
three kinds of object it just made: the blob, the `dir` tree (by one built in a
private index) and the commit itself (by a sibling whose message reads
`replacing-commit`). It pins `core.useReplaceRefs = true` in the repository and,
with `extensions.worktreeConfig`, in the main worktree's own configuration. The
checkout was written before any replacement existed, so the run's clean check,
which reads the recorded graph, passes. It returns the blob's id and the probe's
spec lines.

## `fn assert_every_scope_says_true(repo: &Path) {`

The challenge's own precondition. `git config --show-scope --get-all
core.useReplaceRefs` must list `true` at system, global, local and worktree scope,
and twice at command scope, once for `GIT_CONFIG_COUNT` and once for
`GIT_CONFIG_PARAMETERS`, and no `false` anywhere. A witness whose challenge had
been neutralised away would stop here, not pass.

## `fn configuration_of(repo: &Path, also: &[&Path]) -> Vec<(PathBuf, Vec<u8>)> {`

The bytes of every file directly in the common directory whose name starts with
`config` — `config` and `config.worktree` — and of any other file the caller
names. Compared before and after, it says the run wrote no Git configuration.

## `fn probe_git_past_another_registrations_write(dir: &Path, args: &[&str]) -> Result<String, String> {`

*Proposed and conditional: B-W924-R2, not adopted (the follow-up B record's
§9.25).* The gate probe's `git config --worktree core.useReplaceRefs true`
enumerates the repository's registrations before it writes anything
(`location_options_init` calls `get_worktrees()` first), so in the siblings
witness it can die reading the other sibling's add in flight, at its empty
`commondir`. The B-W924 diagnosis's half-forced runs saw it do so: that probe was
the first reader to meet the held write and exited 128, and a later attempt of
the same run met the add itself (what ended the first attempt was not observed
directly). The product's snapshot commands now run again past
such a read (B-W924-R1, `src/workspace.rs`), and this probe runs again on the
same terms: another registration's `commondir` read at zero bytes, the gate's own
snapshot never, one millisecond's sleep doubling to fifty, and a ten-second
deadline from the first probe, checked after each probe, the probe after a
sleep admitted however late it starts (no hard wall-clock bound). Every
other answer is the probe's own, and the check and its record are unchanged: a
setting that never lands still records `worktree-true FAIL` with Git's message.
It sits after the siblings' helpers so that the witness's own lines keep their
numbers. It reads its own copy of the rule, so that either change can be taken
without the other.

It builds the probe's command itself, `git -C <dir> <args>` with the test
process's environment, as `probe_git` builds it, runs it once per attempt, and
decides each attempt on the attempt's raw output: its exit status and its
standard error as Git wrote them. Only the attempt it returns is converted, by
`probe_answer`, into the probe's answer. (Until the B9 round it ran `probe_git`
itself and decided on that answer, which has lost the exit status and trimmed
the standard error, so a matching line with any failing exit, a signal, no
newline or a blank line after it was attempted again: B-I8-1, the follow-up B
record's §9.26.)

## `fn past_another_registrations_write(`

The loop, against an injected command and deadline, so that its decision runs
without Git (the test below). It decides on each attempt's raw output, before
any conversion, and returns that output; a command that cannot start is
returned at once.

## `fn anothers_empty_commondir_in(output: &std::process::Output, dir: &Path) -> bool {`

The rule as B-W924-R1 stated it (its predicate, `read_anothers_empty_commondir`, was in `src/workspace.rs` until B-W924-R1-S withdrew it) in
a copy of its own: exit 128, Git's death; standard error exactly one line, ended
by its newline; that line Git's `fatal: failed to read `, a path whose last
components are `worktrees` and a registration's name, then `/commondir:
Success`; and the name not the gate's own snapshot directory's, nor that with
anything after it. A directory with no name of its own matches every name, so
nothing is attempted again for it.

## `fn probe_answer(output: std::io::Result<std::process::Output>) -> Result<String, String> {`

The probe's answer, as `probe_git` converts it: a command that cannot start
answers its error's text, a success its standard output, and any other exit its
standard error, trimmed. `probe_git` stays as it was; this is the same
conversion, applied after the decision rather than before it, and
`a_gates_worktree_setting_answers_what_the_probe_answers_when_nothing_tears`
compares the two on Git's own answers.

## `fn gate_probe_writer_helper() {`

Linux only, and ignored: the child the test above runs. In a scratch tree from
`temp_engine_scratch` it calls
`runner::host::tests::inherited_writer::writes_leave_no_writer_in_another_threads_fork`
with a FIFO path whose name holds shell metacharacters, and a closure that
writes the oracle's `shim_script(marker)` there through
`write_the_gate_probe_stand_in`, the writer the gate role's cases use. The FIFO
is what the gate role's section below found missing: its pipe is smaller than
the script, so the write stays open, blocked, while another thread of this
process forks, and the window is entered by construction rather than by racing.
Once the writer is done, the reader sees end-of-file only if that fork holds no
writer of the FIFO. The writer helper writes from a process of its own and leaves
none there; a write from this process leaves its descriptor in the fork, the
read answers `WouldBlock`, and the oracle fails. The oracle reads the inherited
writer, not `ETXTBSY`, since a FIFO cannot be executed: what it shows is that
the stand-in's writer leaves no descriptor for a fork to keep, which is the
writer half of the repair and holds without the isolated child's single test
thread, because the oracle makes its own fork from another thread. The stand-in
writer's `fs::set_permissions` gives the FIFO mode 0700 as it does the stand-in.

## `const TORN_REGISTRATION: &str = "foreign-torn";`

The tests appended by follow-up D (adopted with decision O8 on 2026-10-11 as an agent decision by
`pr11_decide_d` under the owner's delegation of 2026-10-11 (babysit's `0194a1e4…`), and in force
only from #331's merge;
`48f2af7a:reviews/2026-10-02-pr11-follow-up-d-record.md`, its Implementation section) drive the
real legacy engine — `run_with` and `resume_harness` — through a worktree registry another
process has torn. Their residue is a foreign registration under this name: `gitdir`
written as Git writes it (`GitdirRule::native().spelling`) and `commondir` opened and
empty, the state a writer killed while it writes `commondir` leaves. Every enumeration
of the store dies on it. `repair_the_torn_registration` is the operator's repair.

## `fn tear_after_capture(`

The after-capture hooks (`RunOptions::after_candidate_capture`, a test-only seam of the
legacy attempt) record the captured branch, parent and tree beside the repository
(`record_the_capture`, read back by `captured`) and then make the state each test
needs: the residue alone (`tear_after_capture`); the index changed after the capture
(`unstage_and_tear_after_capture`, FUB-D5-INDEX's interleaving); `HEAD` moved after the
capture (`move_head_and_tear_after_capture`, R-D1; since R-D1's part K the kept pin is
written anyway, and T-R3 below asserts its captured identity); a ref blocking the kept pin's
name (`block_the_kept_pin_and_tear_after_capture`, T-P8's trigger since part K); the
snapshot store made impossible (`block_the_snapshot_store_after_capture`); an attempt
error that is not the registry's (`fail_after_capture`); a replacement of the worker's
new file's blob, or of the captured tree by its parent's, made at capture
(`replace_the_new_blob_and_tear_after_capture`,
`replace_the_captured_tree_and_tear_after_capture`, FUD-D1-REPLACE). The residue is
planted after the capture so that the gate or review snapshot is the first registry
access that meets it.

## `fn kept_pins(repo: &Path) -> Vec<(String, String)> {`

Every `-kept` pin under `refs/upstroke/prepared/`, with the commit it names, read with
`for-each-ref`: what the coordinator kept, and what no resume may remove.

## `fn follow_the_restore(repo: &Path, warning: &str, pin: &str) -> String {`

Runs the command the resume's warning advertises for a `HEAD` still at the pin's parent,
exactly as an operator would, from the checkout's root: the warning's backticked
`git …` spans, `<pin>` replaced, the first whose subcommand is `restore` (or round 5's
`checkout`). `follow_the_pick` runs the command it advertises for a later `HEAD`: the
first after the restore that is neither the pin's removal nor `git diff --cached`.
`differences_from_the_pin` compares the checkout's files and index with what the pin
records, read with the replacement controls: every path's bytes, no index path outside
the pin, and the index's tree. The repositories that compare bytes set
`core.autocrlf=false`, because the restore writes working files through the checkout's
own conversions (FUD-D4-RESTOREBYTES).

## `struct ShapedWorker {`

A worker that edits a tracked file, adds one and deletes one before the fake's own edit:
FUB-D5-RESTORE's shape, whose recovery must restore the deletion too.
`struct TearingWorker {` plants the residue from inside a resumed attempt, as any worker
writes beside its checkout, so the resume's own attempt is refused with no seam.

## `const ENGINE_CALL_BOUND: Duration = Duration::from_secs(300);`

The tests of R-D1's preservation design, round 4 (`~/orch-pr11/owner-package/RD1-PRESERVATION-PROPOSAL.md`, sha256
`f9e81c07…`; parts K, G4 and N, adopted with O8 on 2026-10-11 as an agent decision by `pr11_decide_d` under the
owner's delegation of 2026-10-11 (babysit's `0194a1e4…`), and in force only from #331's merge; record §5.17), appended
after D's. Every call into the
legacy engine is bounded: an in-process run or resume by `bounded` (a watchdog thread and a channel's `recv_timeout`, so
a wedged engine fails the test at the bound instead of hanging the suite; `bounded_run`, `bounded_resume`), and a child
of this test binary by `bounded_child` (output to a file, `try_wait` polled, killed and reaped at the bound). The bound
is generous for CI's slowest leg and times no healthy step: every wait inside the tests is a handshake (an after-capture
hook, a child's exit, a shim's marker), never time alone.

## `fn rd1_repo(tag: &str) -> (ScratchTree, PathBuf) {`

The harness's repository shape: `tracked.txt`, `deleted.txt`, a tracked `.gitignore` (`ignored.flag`) and `sub/t.txt`,
with `core.autocrlf=false` because bytes are compared, and one gated task. `kept_pin_of`, `pin_target`, `blob_at`,
`kept_copies`, `restore_the_pin_from` (the `git fetch <copy> <pin>:<pin>` the copy warning advertises) and
`forget_every_unreferenced_object` (every reflog expired, `gc --prune=now`) read what the engine kept.

## `struct ShapingWorker {`

A worker whose output is a named shape written into its checkout (`shape_named`: the harness's cases, from the paid
output to an ignored directory where `HEAD` has a file, a nested repository, an unreadable file, a CRLF output or a name
that is not UTF-8), the fake adapter's own run then adding nothing. With `then_exit` the process dies right after the
shape is written: `die_inside_an_attempt` runs the legacy engine that way in a child of this test binary
(`kept_shape_child_dies_inside_an_attempt`), the killed-mid-attempt pattern of
`killing_a_run_mid_attempt_leaves_a_resumable_record`, so the log ends with the attempt in flight and nothing captured or
pinned. `resume_and_die_inside_the_attempt` does the same to a resume, after its guarded discard ran, so a second resume
finds an attempt in flight again. `UnparsedWorker` writes the output and then fails the adapter's parse.

## `fn a_kept_pin_is_written_whatever_head_is_when_the_snapshot_is_refused() {`

The tests of R-D1's own cases, each red at D's head as the merge has it: T-R1
(`a_branch_moved_before_capture_and_back_keeps_the_output_through_the_first_resume`, A1 and A1t — the worker moves the
branch, with or without a foreign change in the moved commit's tree, and the hook moves it back), T-R2 (A1h, put back
with `reset --hard`), T-R3 (A2, A2h: the refusal names `git update-ref`; T-P8's FUB-D5-INDEX role), T-R4 (A3 to A6:
detached, another branch, the branch deleted, its ref symbolic — the tear repaired before the operator's restore, which
lists the worktrees), T-R5 and T-R6 (B1, B2; Unix, mode bits), `a_store_fault_at_the_pins_objects_or_its_reflog_loses_nothing`
(B3, B4; an object store that takes no object fails the gate snapshot's own commit first, so that run ends through N1's
arm, its pin failing too), T-R7 (C1, C2, C4), T-R8 (C3), T-R9 (E1, and E2 with a shim that kills the run at the pin's
`update-ref`), T-R10 (F1, F2: `update-ref -d`, and a `fetch --prune` from a bare remote), T-R11 (X3) and T-R12 (X2, the
control). Each pinned output is checked by its tree, and the warning's restore is followed where the case resumes.

F2's fetch runs with `-c maintenance.auto=false -c gc.auto=0`, two of the four settings the engine's own Git children
carry (`AUTO_MAINTENANCE_REFUSED`, `src/workspace.rs`). From Git 2.54 a fetch's automatic maintenance takes the
`geometric` strategy, whose `worktree-prune` task removes the torn registration: its `gitdir` names a checkout that does
not exist, and it has no index. The maintenance detaches, so it races the operator's repair, and where it wins the
repair finds nothing to remove. That is how T-R10 failed on the hosted runner's Git 2.55.0. Without the two settings,
Git's maintenance policy, not the engine, would decide what the repair finds. F2's case, the pin pruned by
`fetch --prune`, is unchanged.

## `fn inherited_selection_run_helper() {`

The child of the test above.

## `fn absolute_common_dir_run_helper() {`

The child of the test above; its body is `gated_run_under_an_inherited_common_dir`.

## `fn relative_common_dir_run_helper() {`

The child of the test above.

## `struct SharedOwnerWitness(std::sync::Arc<OwnerWitness>);`

`OwnerWitness` behind an adapter source that can be shared. Production's `BuiltinAdapters` is
shareable, so `upstroke run` reaches the concurrent driver (`run_concurrently`); the step loop is
what an unshareable source gets. The acceptance record therefore runs on a shareable source, and
this wrapper keeps the owner witness on it. Each `AgentAdapter::probe` made through the source
still notes whether `owner.json` was already published under the private root. That is
everything the witness observes: it does not see the shell probe or any probe made another way.

## `const HOST_ACCEPTANCE_MARKER: &str = "UPSTROKE-HOST-ACCEPTANCE-RECORD";`

The first field of every line of the record's framing. A reader finds the record by it, after any
timestamp a CI log puts in front of the line. It is never the verdict: see
[`host_acceptance_record`].

## `const HOST_ACCEPTANCE_FRAMING: &str = "v1";`

The framing's version, the second field of every line. A reader that knows only `v1` refuses any
other.

## `const HOST_ACCEPTANCE_CHUNK: usize = 96;`

Record bytes per chunk line. A chunk line is 192 lowercase hex characters after a prefix of about
55, so no line passes about 250 characters, and one large payload never sits on one line.

## `const HOST_ACCEPTANCE_CI_VARIABLES: [&str; 15] = [`

The only environment names the record reads. It reads them by name and enumerates nothing else.
They are provenance, never proof: the CI repository, workflow, event, commit, ref, run, attempt,
job and runner (`RUNNER_*`, and `ImageOS`/`ImageVersion` where GitHub's hosted images set them).
Whoever binds the record to a CI job checks the checkout and source against that job and the
reviewed head. Environment text is not independent evidence of either.

## `const HOST_ACCEPTANCE_FILES: [(&str, &str); 4] = [`

Each run's four records, and the half of the run that holds each: `events.jsonl` and `report.json`
in the public directory, `owner.json` and `committed.json` in the private one. The record's
`manifest` lists them in this order, per run.

## `const HARNESS_GIT_CONTEXT: [&str; 10] = [`

The ten names that select another repository's Git context. [`harness_git`] removes them from its
child, so an inherited `GIT_DIR` (or common, object or configuration selector) cannot make the
record name a repository other than the one the test binary was built from.

## `fn host_acceptance_sha256(bytes: &[u8]) -> String {`

Lowercase hex SHA-256, the digest every field of the record uses.

## `fn host_acceptance_hex(bytes: &[u8]) -> String {`

Lowercase hex, two characters a byte, with no dependency: no base64, and nothing a log can
reinterpret.

## `fn harness_git(args: &[&str]) -> String {`

Git run with `-C` set to `CARGO_MANIFEST_DIR`, the checkout this test binary was compiled from,
and never in a scratch run repository. [`HARNESS_GIT_CONTEXT`] is removed from the child's
environment. It refuses on a failed Git command.

## `fn host_acceptance_current_exe() -> serde_json::Value {`

The running test binary by its own bytes: its file name, byte length and SHA-256. The hash is
streamed, so a large debug harness is never held in memory, and the length is checked against
the file's metadata, so a short read cannot pass as the whole binary.

## `fn host_acceptance_harness() -> serde_json::Value {`

What built and ran the record: crate name and version, `std::env::consts` OS, architecture and
family, [`host_acceptance_current_exe`], and the harness checkout's `HEAD`, `HEAD^{tree}` and
whether `git --no-optional-locks status --porcelain --untracked-files=no` printed anything. When
`tracked_changes` is true, the compiled source may differ from `HEAD`. A CI checkout reports
false.

## `fn host_acceptance_ci() -> serde_json::Value {`

[`HOST_ACCEPTANCE_CI_VARIABLES`], each as a string or null, and a `state`. A run with none of the
names set says `absent` and states that the record was not made by a GitHub Actions job. A local
capture says this explicitly rather than leaving the fields blank.

## `struct HostAcceptanceParked {`

What the resumed run's parked epoch left before it was answered: the log's length and digest, and
`owner.json`'s digest.

## `fn host_acceptance_run(`

One released run's records, asserted and then returned as its part of the record. These are the
assertions `a_schema_4_run_publishes_its_records_in_order_and_records_one_runner` and
`a_schema_4_run_is_created_driven_and_merged_through_the_public_entry_point` make, applied here
to both runs:

- `owner.json` was published before every adapter probe the owner witness saw, the first
  included — each `AgentAdapter::probe` the run made through its adapter source, and nothing
  more;
- the first event is `run_started` with schema 4, the full run id, the run's branch, runner kind
  `host` and no image;
- `committed.json` names `sha256:` plus the digest of the committed first line's bytes, and its
  run id and incarnation;
- one runner, serialized byte for byte, in `run_started`, `owner.json`, every `run_resumed` and
  `report.json`;
- `report.json` is the fresh projection of the committed log;
- exactly two `task_merged` events, and the recorded integration ref
  (`refs/heads/upstroke/run-<run_id>`) holds the last merge the log names;
- the log ends with `run_finished`, complete.

For the resumed run it also asserts that the final log begins, byte for byte, with the log the
parked epoch left, and that `owner.json` is unchanged by the resume. A fresh run records no
resume, and a resumed run records at least one. The run's files are embedded as their exact UTF-8
text, with `half`, `bytes` and `sha256`; the test asserts they are UTF-8 rather than choosing
another encoding.

## `fn host_acceptance_fresh() -> serde_json::Value {`

The complete fresh run: `engine::run_with` with `topology_options` over the fixture's two-task
plan (`t2` depends on `t1`). The source is [`SharedOwnerWitness`] over `TopologyStub` asking no
question. The run must be complete, with both tasks committed and the stub's edit on the
integration branch. `until_released` runs before anything is read.

## `fn host_acceptance_resumed() -> serde_json::Value {`

A separate run that parks on its one question and is answered and resumed:

1. `engine::run_with` parks, and the run is released.
2. The parked log and `owner.json` are read.
3. `crate::answer::answer` records `"the first"`, and the run is released again.
4. `engine::resume_with` completes it (`private_root` left to the log, as the PR12 resume tests
   do), and the run is released once more before its records are read.

The resume must keep the run id, complete the run and commit both tasks.

## `fn host_acceptance_frame(record: &[u8]) -> String {`

The record's bytes R as lines a CI log keeps intact, all ASCII and single-spaced:

```text
UPSTROKE-HOST-ACCEPTANCE-RECORD v1 BEGIN <tag> <N> <B> <S>
UPSTROKE-HOST-ACCEPTANCE-RECORD v1 <tag> <i> <hex>        (i = 1..N, in order)
UPSTROKE-HOST-ACCEPTANCE-RECORD v1 END <tag> <N> <B> <S>
```

- `B` is R's length and `S` its SHA-256.
- `tag` is `S`'s first sixteen characters, which can never read `BEGIN` or `END`.
- `N` is `ceil(B / 96)`.
- Every chunk but the last is exactly 192 hex characters.

With these rules each kind of damage is detectable:

- a wrapped line breaks a length;
- a truncated one breaks a length or the count;
- a lost or repeated line breaks the contiguity;
- a changed byte breaks `S`.

## `fn host_acceptance_record() {`

G7's native HOST acceptance record (root direction `087fea89`), the host half of R21's per-platform
run ids. CI selects it by its exact name after each category leg's share of the suite: once in
each of `test-linux`'s 7 legs on `ubuntu-latest`, `test-macos`'s 9 on `macos-latest`, and
`test-windows-hosted`'s 20 on `windows-latest`. The categories are `kill-path`, `coordinator`,
`engine`, `workspace`, `runner`, `unit` and `self-checks`; the ordered `TEST_CATEGORIES` registry
in `src/effects/tests/ci_model.rs` generates their platform partitions. Check names are
`test (<runner>, <category> [k of n] (<subcategories>))`, with `k of n` only for a split category.
The `upstroke-ci` aggregate requires every leg and the lint and MSRV jobs (`MAINTAINING.md`
step 3). The capture remains a separate step running the one ignored test:

```text
cargo test --all-features --lib engine::tests::host_acceptance_record -- --ignored --exact --nocapture
```

It is ignored everywhere else and compiled on all three platforms, with no `cfg` and no early
return. It drives a complete fresh run and a separate parked, answered and resumed run. Both are
real schema-4 runs on the production `HostRunner`, through the public entry points, with the
deterministic stub adapter. Every assertion of [`host_acceptance_run`] holds of both, and the two
run ids differ, before a byte is printed.

The record is one compact JSON document:

- `format`, `format_version` and `test`;
- `harness` ([`host_acceptance_harness`]);
- `ci` ([`host_acceptance_ci`]);
- `scope`, which says plainly what the record does not show, including what the owner witness
  does and does not observe;
- `runs`: each with its full run id, invocations, recorded runner, epochs, the owner witness's
  list (`owner_published_at_each_adapter_probe`), first line, integration ref and sha, merges, and
  the four raw files;
- `manifest`: one row per embedded file.

`format_version` is 2. Version 1, written only by the development capture before it, named the
owner list `owner_published_at_each_probe` and carried no `scope.owner_witness`. The rename says
what the list is.

It goes to stdout in [`host_acceptance_frame`]'s lines through one `write_all`. The engine forbids
`println!` (`#![forbid(clippy::disallowed_macros)]` in `engine`), and `v1_role_probe_reviewer`
writes its marker the same way under this file's `disallowed_methods` allow.

**Printing is not the verdict.** A record is accepted only when all of these hold:

- the capture command exited 0;
- its output holds this test's pass line and `test result: ok. 1 passed; 0 failed; 0 ignored;`;
- the frame is complete;
- the reconstruction's length and SHA-256 match;
- every embedded file's length and digest match its fields and its manifest row;
- the identities re-derived from the raw files agree.

The CI step checks the first three of these as it runs. The decoder that binds a job's log checks
all of them, and only over the capture step's executed output. It must be given either that
bounded stream, or a job log from which it extracts the capture step's group, found by the pinned
script the runner echoes. The echo itself is not output and its marker literals are not frame
lines, and the suite step's many `1 passed` summaries from child harnesses are not the capture's.

**Limits.**

- Host only: the runner kind is `host` and `image` is null by design. No container is used, and
  container capability on the macOS and Windows runners stays unknown.
- The owner witness observes only the adapter probes made through the adapter source. It is not
  a direct observation of the shell probe, or of every probe the run makes.
- No provider is called.
- The coordinator is this test process calling `engine::run_with`, `answer::answer` and
  `engine::resume_with`, not the `upstroke` CLI process.
- The scratch repositories are deleted when the test ends. The embedded files are the retained
  evidence, and how long they last is bounded by CI log retention.

## `const CONTAINER_ACCEPTANCE_MARKER: &str = "UPSTROKE-CONTAINER-ACCEPTANCE-RECORD";`

The first field of every line of the container record's framing. It differs from
[`HOST_ACCEPTANCE_MARKER`] so that a reader of either record never takes the other's lines. It is
never the verdict: see [`container_acceptance_record`].

## `const CONTAINER_ACCEPTANCE_FRAMING: &str = "v1";`

The container framing's version, the second field of every line. A reader that knows only `v1`
refuses any other.

## `const CONTAINER_ACCEPTANCE_IMAGE: &str = "UPSTROKE_CONTAINER_ACCEPTANCE_IMAGE";`

The one environment name the container record requires: the image reference both runs configure
as `[runner] image`. It has no default. The record refuses when it is unset, and it pulls nothing.
Each platform recipe names an image the platform's daemon already holds: macOS the digest
reference its setup probe pulled, Windows the setup's `alpine:3.20`.

## `const CONTAINER_ACCEPTANCE_DIR_TARGET: &str = "/upstroke-acceptance/dir";`

The container targets of the two configured mounts, the configured file's text, and the
markers the stub's probe, worker and reviewer print. They are this constant and the eight after
it: `CONTAINER_ACCEPTANCE_FILE_TARGET`, `CONTAINER_ACCEPTANCE_FILE_TEXT`,
`CONTAINER_ACCEPTANCE_PROBE`, `CONTAINER_ACCEPTANCE_WORKER`, `CONTAINER_ACCEPTANCE_REVIEW` and
`CONTAINER_ACCEPTANCE_REVIEW_WRITE`, the file a reviewer tries, and is expected not, to create in
its checkout.

## `fn container_acceptance_write_attempt(path: &str) -> String {`

A shell fragment that tries to create `path` and prints `allowed` or `refused`. A role that should
not write reports what the boundary did, rather than failing the stub's command.

## `fn container_acceptance_probe_script() -> String {`

The agent probe, run by the image's `sh` in the probe container's scratch directory. It prints the
process's uid and gid, `uname`'s system, release and machine, and whether the root and the
configured directory refuse a write (both must, a probe being read-only throughout). It also prints
the configured file's text.

## `fn container_acceptance_worker_script() -> String {`

The worker: the topology stub's edit (`TOPOLOGY_STUB_EDIT`, which `sh` runs as it does on the host)
in the bind-mounted checkout, then one line of what the container sees. That line has its uid and
gid, the checkout's `README.md`, the configured file, and the role Git view's `HEAD` and
`objects/info/alternates`, found through the checkout's `.git` pointer. The worker then appends one
line to the configured directory, so the host can count worker containers by what reached it.
The image needs `sh`, `cat`, `sed` and `id` and no Git: the view is read as files, the way the
engine materialized it.

## `fn container_acceptance_review_script() -> String {`

The reviewer: a write to its checkout and to the configured directory, each expected `refused`
(the checkout is read-only for a reviewer and configured mounts are forced read-only), the
configured file, its uid and gid, then the stub's passing verdict.

## `fn container_acceptance_gate_script() -> String {`

The one `[[gates]]` command, run by the run's recorded shell (`sh`) in the gate's container: the
stub's edit must be in the checkout, and the gate appends one line to the configured directory.

## `fn container_acceptance_config(image: &str, directory: &Path, file: &Path) -> String {`

The run's `upstroke.toml`. It sets `[engine] shell = "sh"`, so the shell probe and the gate run
under the image's shell rather than the native one, which is `cmd` on Windows. It selects
`[runner] kind = "container"` with the image. It configures one writable directory mount and one
read-only file mount, and the gate. Configured mounts are read-only unless they say otherwise, so
the directory says `read_only = false`. Every value is a TOML literal string, so a Windows path's
backslashes need no escaping. A value that cannot be one is refused.

## `struct ContainerAcceptanceFixture {`

The image reference and the configured mounts' sources, created beside the repository inside its
guarded scratch tree (never inside the checkout or the private root, which the mount rules
exclude), with the config committed with the plan. On macOS the recipe points `TMPDIR` into the
VM's shared directory, so every fixture the containers bind is one the VM sees at the same path.

## `fn container_acceptance_fixture(`

Creates the [`ContainerAcceptanceFixture`] and commits the config beside the plan
[`temp_engine_repo`] seeded.

## `struct ContainerWitness {`

The adapter both container runs use. It reuses `TopologyStub`'s `parse` and question counter,
but spells every command for the Linux image's `sh` (`ShellKind::Sh`), never the native shell.
`TopologyStub` itself, which the HOST record uses, is unchanged. Like [`OwnerWitness`] it notes
at each adapter probe whether the run's owner record was already published. It also keeps every
output it saw: each probe's, worker's and reviewer's code, stdout and stderr.

## `impl ContainerWitness {`

`new` builds the witness over a fresh `TopologyStub` asking `questions` questions; `witness`
records one output, refusing (never ignoring) a poisoned lock.

## `impl AgentAdapter for ContainerWitness {`

`probe` runs [`container_acceptance_probe_script`] through the runner it is handed, the run's
`ContainerRunner`, so the probe is a container. `build` gives a reader the review script and a
writer the worker script. `parse` records the output, then hands it to the stub.

## `struct SharedContainerWitness(std::sync::Arc<ContainerWitness>);`

[`ContainerWitness`] behind a source that can be shared, as production's `BuiltinAdapters` is, so
both runs take the concurrent driver, as `upstroke run` does.

## `struct ContainerAcceptanceParked {`

What the resumed container run's parked epoch left before it was answered: the log's length and
digest, and `owner.json`'s digest.

## `struct ContainerAcceptanceCase<'a> {`

One container case's inputs to [`container_acceptance_run`]. The `calls` are the public calls in
order, each with the half-open range of journal runtime indices it constructed and, for
`engine::run_with` and `engine::resume_with`, its [`container_acceptance_end_state`].

## `fn container_acceptance_creates(`

The journal's `create` operations made by runtimes in an index range.

## `fn container_acceptance_end_state(`

Right after one public call has returned and released, two things must hold. The daemon lists no
container labelled with the call's private root: the label value is the one the call's own
planned specs carry. And no container intent remains in the private root's intent directory.
This is an end state taken from the daemon and the disk, not a sample of events.

## `fn container_acceptance_fields(`

The `key=value` fields of the first stdout line that begins with a marker; a missing line fails.

## `fn container_acceptance_role(invocation: &str) -> &'static str {`

A container's role, read from its invocation id: `p.shell.*`, `p.agent-*`, `*.worker.*`,
`*.gate*`, `*.review_*`. Anything else fails, so a new kind of container cannot pass unclassified.

## `fn container_acceptance_owner(path: &Path) -> serde_json::Value {`

The host-side owner of a file the containers wrote, recorded rather than asserted. On Unix it is
uid, gid and mode, which on macOS is also what virtiofs presented of the VM's write. On Windows
there is no uid or gid, only the read-only flag. Two definitions, `cfg(unix)` and
`cfg(not(unix))`, which every CI test and Clippy leg compiles on its own platform.

## `fn container_acceptance_lines(path: &Path) -> usize {`

A file's line count, zero when it is absent.

## `struct ContainerAcceptanceRecorded<'a> {`

What every container of one run is checked against: the case, the run id, the recorded image id
and RunnerPolicy digest, and the journal's operations.

## `fn container_acceptance_container(`

One container, validated from the journal. Each of these must hold:

- the planned spec names the recorded image id, and the daemon's create reported it;
- `owner.json` was published before the container was created (probe containers included, which
  the adapter-level witness cannot see);
- the daemon's own inspection at creation reports the recorded image id, the container's name,
  every planned label, a read-only root, the planned user (Unix: the coordinator's uid:gid;
  Windows: the image's default, an empty `User`) and working directory;
- the daemon asked for and holds every planned mount, with no unplanned one. Each bind and
  volume is held with the planned read-write flag. A tmpfs is checked as asked for, since a
  daemon may not list it among held mounts. Bind sources equal the planned spelling except on
  Windows, where the vendor translates the native path; there the client's argument, what the
  daemon asked for and what it holds are all recorded;
- the intent read back from disk, as the observer parsed it into the runner's own record type,
  names this run, the epoch's incarnation, the invocation the container is labelled with, and the
  recorded RunnerPolicy's digest, which the labels do not carry;
- the container started, was collected with exit 0, was removed with its process gone, and was
  stopped at most once by its release, between collection and removal;
- the inspection just before its removal is of the same container id and recorded image, exited
  with the collected code, and had been started;
- the create, start, collection and removal are in that order in the journal.

It returns the container's summary, with the digests of both inspections and of its intent.

## `fn container_acceptance_traced(`

The container names one runtime's own trace (its `DockerCli`'s recording `ContainerTrace`) shows an
effect asked about.

## `fn container_acceptance_journaled(`

The container names the journal shows one runtime asked an effect about.

## `fn container_acceptance_run(input: &ContainerAcceptanceCase<'_>) -> serde_json::Value {`

One released container run, asserted and returned as its part of the record. Every assertion
[`host_acceptance_run`] makes of a host run is made here with the container runner in its place:
`run_started(4)` with schema 4 and shell `sh`, and a `container`/`container-v1` runner recording
the configured reference, a non-empty image id, no credential volumes and both configured mounts.
One runner, byte for byte, in `run_started`, `owner.json`, every `run_resumed` and `report.json`.
`committed.json`, the fresh report projection, two merges, the integration ref, a complete end,
and for the resumed run the byte-for-byte log prefix and unchanged `owner.json`. Then, from the
journal:

- **complete enumeration.** Each `engine::run_with` and `engine::resume_with` constructed exactly
  three runtimes in the schema-4 entry: two at two distinct inspection sites, serving no run, and
  then the run's container runner. `answer::answer` constructed none. Every journaled runtime
  belongs to a call. For every runtime and every effect (create, start, stop, removal), its own
  trace and the journal name the same containers, and an inspection runtime has none at all. So
  no container was created, started, stopped or removed that the record does not hold;
- **epochs.** One runner per recorded epoch, serving that epoch's incarnation. Every container it
  created is validated by [`container_acceptance_container`]. Each epoch probed the shell and the
  agent inside the image, and removed both before its first work container. Its worker containers
  are exactly the `attempt_started` events of that epoch;
- **behaviour across the boundary.** The adapter saw one output per agent probe, worker and
  reviewer container. Each probe ran on `Linux` with the root and the configured directory
  refusing writes. Each worker read the checkout, the read-only file and a Git view whose `HEAD`
  is a commit of this repository. Each reviewer's checkout and configured directory refused its
  writes. Every process ran as the planned user where one is planned. The host holds one line per
  worker container and one per gate container in the configured directory, no read-only role's
  file, and the read-only file unchanged, and no reviewer write reached the integration branch.

It returns the run's identities, the containers and their counts, the witness's outputs, the
host-side effects with their owners, the whole journal snapshot, and the four raw files.

## `fn container_acceptance_fresh(image: &str) -> serde_json::Value {`

The complete fresh container run: [`container_acceptance_fixture`], then `engine::run_with` with
`topology_options` and [`SharedContainerWitness`] asking no question, with the seam armed for that
call only. The run must be complete, with both tasks committed and the stub's edit on the
integration branch. `until_released` runs before the end state and the records are read.

## `fn container_acceptance_resumed(image: &str) -> serde_json::Value {`

A separate container run that parks on its one question: armed `engine::run_with`, the end state,
the parked log and `owner.json` read, unarmed `answer::answer`, then armed `engine::resume_with`
(`private_root` left to the log) and its end state. Every call is released before the next. The
resume must keep the run id, complete the run and commit both tasks.

## `fn container_acceptance_frame(record: &[u8]) -> String {`

[`host_acceptance_frame`]'s line shape, with [`CONTAINER_ACCEPTANCE_MARKER`] as the first field. The
`BEGIN`/`END` summary, chunk size (`HOST_ACCEPTANCE_CHUNK`), tag, count, length and digest rules
are the same, and so is every damage they detect.

## `fn container_acceptance_record() {`

G7's native CONTAINER acceptance record: M2's per-platform Container run ids, recorded
RunnerPolicy and image identity, report identity, and the daemon's own inspection of every actual
container across its actual epochs. Only the platform recipes select it, by its exact name, never
a required job:

```text
UPSTROKE_CONTAINER_ACCEPTANCE_IMAGE=<reference> cargo test --all-features --lib \
  engine::tests::container_acceptance_record -- --ignored --exact --nocapture
```

It is ignored everywhere else and compiled on all three platforms. It refuses, and never skips,
when the image variable is unset, the runtime does not answer, the daemon does not run Linux
containers, or the image is not already held. It drives [`container_acceptance_fresh`] and
[`container_acceptance_resumed`]: real schema-4 runs on the production `ContainerRunner` over the
production runtime, through the public entry points, with deterministic stub adapters. That is
public APIs with stub providers: no provider is called and no CLI process restarts. Every
assertion of [`container_acceptance_run`] holds of both runs, their run ids differ, and both
record the image id the daemon reports for the reference, before a byte is printed.

The record is one compact JSON document:

- `format` (`upstroke-container-acceptance-record`), `format_version` 1, and `test`;
- `harness` ([`host_acceptance_harness`], reused unchanged) and `ci` ([`host_acceptance_ci`]);
- `platform`: the coordinator's OS, architecture and family, the daemon's fixed-field version and
  info answers and current context, and a statement naming the native coordinator and the Linux
  daemon its containers ran in. A Linux daemon serving a macOS or Windows coordinator is
  recorded as exactly that, never as native container execution;
- `image`: the variable, the reference, the id and the daemon's image inspection;
- `scope`: what the runner, observer, provider and coordinator were, how the record is accepted,
  and what it does not claim;
- `runs`: each run's [`container_acceptance_run`] value;
- `manifest`: one row per embedded file.

It goes to stdout in [`container_acceptance_frame`]'s lines through one `write_all`, as the HOST
record does. **Printing is not the verdict**: acceptance needs the capture's zero exit, this
test's pass line and `test result: ok. 1 passed; 0 failed; 0 ignored;`, a complete frame, a
reconstruction whose length and SHA-256 match, every embedded file and inspection matching its
digest, and identities re-derived from the raw files that agree. The recipes check the first four.
The judgement of the native evidence is root's.

## `fn the_container_runtime_seam_observes_only_while_armed_and_only_on_its_own_thread() {`

The seam's scope, in the ordinary suite with no runtime needed, since constructing a `DockerCli`
starts nothing. An unarmed construction is not observed. An armed one on the arming thread is,
with its call site and, for a runner, its identity. Another thread's construction while armed is
not. Nothing is observed once the guard drops, and a construction performs no operation.

## `enum PrivateEntry {`

What one path under a private root is, for [`private_snapshot`]: a directory or a
file with its permission bits (on Windows, the read-only flag), the file with its
bytes, or a link with its target.

## `fn private_snapshot(root: &Path) -> std::collections::BTreeMap<PathBuf, PrivateEntry> {`

Every persistent entry under a private root, the root itself included, keyed by its
path relative to the root. A refused resume that published the include, created its
`git` directory, left a staged temporary, renamed anything into place or tightened a
mode changes it; [`moved_entries`] names what moved without printing any bytes.

[`snapshot_under`] with no case: every read that fails still fails the test, now
naming the snapshot's top, the path that failed and what was being read.

## `fn private_snapshot_in(root: &Path, case: &str) -> std::collections::BTreeMap<PathBuf, PrivateEntry> {`

[`private_snapshot`] whose failures also name the case: what the guards of a matrix
take, so a read failure says which of its cases it struck.

## `fn snapshot_under(`

The one walk behind both snapshots: `symlink_metadata` on every entry, never
followed, a directory by its mode, a file by its mode and bytes, a link by its target;
`outside_git` leaves the checkout's top-level `.git` out. **Every read that fails
still panics** — nothing is skipped, ignored or retried — and the message names the
case, the snapshot's top, the path and the read (PR12-R12-WINDOWS-HUSK-SNAPSHOT-LOCK:
Native16's `os error 33` named neither the file nor the case).

## `fn put_include(root: &Path, state: IncludeState) {`

The two include states the CI-01 guard holds a refusal to: `Absent` removes the root's
whole `git` directory, so a premature write would have to create a directory, a staged
file and the include; `Changed` leaves other bytes in a directory loosened to `0755`
on Unix, so a premature write would also rename over them and tighten the directory
to `0700`.

## `fn recorded_shell_helper() {`

The recorded-shell guards' child. `create` makes and parks a zero-gate run under the
config it is given; `resume` resumes the latest run under the config it is given and
asserts that it appended `run_resumed`. Without its environment it does nothing.

## `const TOPOLOGY_STUB_ASKS: &str = "UPSTROKE-TOPOLOGY-STUB-ASKS";`

What a topology stub worker prints to ask its question every time it runs.
`TopologyStub::parse` answers such an output with `TOPOLOGY_STUB_QUESTION`,
whatever its `questions_left` counter says, so a test can choose by task which
worker asks; the counter still decides for every other output.

## `fn announcing_worker(gate: &Path) -> String {`

The J11 worker script, chosen by the task slot it runs in, as
`CONFLICTING_WORKER` chooses: the second task (`k1-*`, `asks`) prints
`TOPOLOGY_STUB_ASKS`; the third (`k2-*`, `waits`) writes `waits-started` in the
gate directory and then waits, at most four minutes, for the test to create
`release` there, failing if it never comes; the first (`first`) only edits.
Each writes inside its own predicted region, so `asks` and `waits` hold
disjoint leases and run together at width 2.

## `fn topology_announcing_child() {`

The child of the test above: the fresh run (`run_with`), or the resume through
the recorded locator with today's config and a raised budget (`resume_with`),
under the driver the parent names, with a 600-second attempt timeout so the
held sibling is never timed out. It asserts the run ends parked on `asks` alone,
with `first` and `waits` committed.

## `const UNSPELLABLE_HOME_REPO: &str = "UPSTROKE_UNSPELLABLE_HOME_REPO";`

The variable that hands the J14 child its repository.

## `fn unspellable_default_root_child() {`

The child of the test above: a fresh run through `run_with` with no private
root, so the root is the default one under the child's `HOME`. It asserts the
refusal is the default root's own, says the root is not valid UTF-8 and that
nothing was locked or created, and that no run directory exists.

## `const INHERITED_COMMON_REFUSALS: &str = "UPSTROKE_INHERITED_COMMON_REFUSALS";`

The variable that names, to J10's child, the file it writes both refusals to.

## `fn inherited_common_dir_mount_child() {`

The child of the test above. `run_with` over the container configuration is
refused with `[runner] the mount … is refused`, its source lying inside the
canonical common directory, authoritative Git; `resume_with` on the parked run
is refused as recording that mount, for the same reason, with nothing
published, locked or written. It then writes both refusals to the file its
parent names, the evidence that it ran.

## `const RECORDED_TARGET_REFUSAL: &str = "UPSTROKE_RECORDED_TARGET_REFUSAL";`

The variable that names, to K1's child, the file it writes its refusal to.

## `fn recorded_target_resume_child() {`

The child of the test above: `resume_with` on the parked run is refused, and
the refusal is written to the file its parent names, the evidence that it
ran.

## `fn stub_pool_file(tree: &ScratchTree) -> PathBuf {`

One pool, `claude-max`, for the topology stub's agent (`claude-code`), so a run's attempts and
review passes are attributed to it. The file is in the scratch tree, outside the checkout: a
schema-4 run refuses a dirty tree (L2), and an untracked pools file would be dirt.

## `enum ExclusionState {`

The two run-exclusion states the lease guards hold a refusal to, beside [`IncludeState`]'s two.

## `fn put_exclusion(repo: &Path, state: ExclusionState) {`

`Absent` removes `.upstroke/.gitignore`, and `.upstroke` itself when nothing else is in it, so a
premature exclusion would have to create the directory too; `Changed` leaves a file holding bytes
with no `*`, which `Workspace::ensure_run_exclusions` rewrites, so a premature one would show as
changed bytes.

## `fn public_snapshot(repo: &Path) -> std::collections::BTreeMap<PathBuf, PrivateEntry> {`

[`private_snapshot`]'s walk over the checkout, without its Git directory: every operator file and
everything under `.upstroke` — the exclusion, every run's public half, its log, `run.lock` and
`cleanup.lock` — by path, kind, mode and bytes. `.git` is left out because its refs are
[`every_ref`]'s, and the lease file in it is created by whoever takes the lease first; the guards
below compare refs instead.

## `fn public_snapshot_in(repo: &Path, case: &str) -> std::collections::BTreeMap<PathBuf, PrivateEntry> {`

[`public_snapshot`] whose failures also name the case.

## `fn assert_admitted_and_published(`

The positive control, run once the hold is released: with the exclusion and the include both
absent, the same public command is admitted, and afterwards the exclusion holds its fixed `*\n`,
the include its fixed bytes, and on Unix the include's directory is `0700`. It returns the report,
whose run is released before it returns.

## `const DEFAULT_ROOT_LEASE_REPO: &str = "UPSTROKE_DEFAULT_ROOT_LEASE_REPO";`

The repository the default-root lease child runs in, named by its parent.

## `fn default_root_lease_child() {`

The child of the test above, with no `--private-root`. Its premises: the default root resolves
under the child's home and does not exist yet. Under the lease it holds in-process, a fresh run
through `run_with` is refused as already driven, builds nothing, and leaves the home (the would-be
root's parent, walked by [`private_snapshot`]), the checkout outside `.git` and every ref exactly as
they were. Released, the same run is admitted and completes: the include is under the root it
created, the run's recorded private half is under that root canonicalized
(`run_started(4).private_dir`), and the run exclusion is made.

Measured before commit as supplementary negative control A1b (A1's old ordering, applied again):
the refused run created the default root itself, its `git` directory and the include under the
home ("created .upstroke", "created .upstroke/git", "created
.upstroke/git/recorded-objects.gitconfig"), and the child and this test fail.

## `fn with_initial_read_failing<T>(log: &Path, call: impl FnOnce() -> T) -> (T, usize) {`

`call`'s result with every initial complete read of `log` failing on this thread
(`entry::initial_log_read_fault`), and how many such reads it attempted. The fault is disarmed
when this returns, and on unwinding.

## `fn assert_the_initial_read_refused(refused: Option<UpstrokeError>, log: &Path, case: &str) {`

The refusal the guards below require: `UpstrokeError::Io` naming `log`, whose source is the
injected failure's own message, so neither another read's error nor another refusal stands in
for it.

## `const DEFAULT_ROOT_UNREAD_REPO: &str = "UPSTROKE_DEFAULT_ROOT_UNREAD_REPO";`

The repository the default-root unread child resumes in, named by its parent.

## `const DEFAULT_ROOT_UNREAD_RUN: &str = "UPSTROKE_DEFAULT_ROOT_UNREAD_RUN";`

The parked run that child resumes.

## `const DEFAULT_ROOT_UNREAD_DONE: &str =`

The line the child says on stderr once its last assertion has passed. The child returns early,
and still reports `1 passed`, when its parent's variables are absent (an `--ignored` run of the
whole suite), so its parent requires this line as well: a child that skipped cannot produce it.

## `fn default_root_unread_resume_child() {`

The child of the test above, resuming with no `--private-root`. Its premises: the default root
resolves under the child's home and does not exist yet. With no default root, and then with one
holding an include of other bytes in a directory the writer would tighten, under the lease it holds
in-process and with the router's initial read failing, the resume is refused with the read's own
error after one attempt, builds nothing, and leaves the home (walked by [`private_snapshot`]), the
checkout outside `.git` and every ref exactly as they were. Read again and released, the resume is
the topology's: it is admitted and publishes under the root the run recorded
(`assert_admitted_and_published`), and it leaves the home, today's default root included, exactly
as it was, since today's default is not a resume's authority.

Measured before commit under the same negative control B1: with no default root, the refused
resume created the default root, its `git` directory and the include under the home ("created
.upstroke", "created .upstroke/git", "created .upstroke/git/recorded-objects.gitconfig"), and the
child and its parent fail.

## `const WORKTREE_LEASE_CONTENDER: &str = "engine::tests::worktree_lease_contender";`

The helper [`contend_for_the_worktree_lease`] runs in a process of its own.

## `const CONTENDER_REPO: &str = "UPSTROKE_TEST_CONTENDER_REPO";`

The checkout whose lease the contender asks for.

## `const CONTENDER_GIT_DIR: &str = "UPSTROKE_TEST_CONTENDER_GIT_DIR";`

Its worktree Git directory, so the contender asks with `WorktreeLock::acquire_in` and runs no Git
command of its own: nothing it does can change `.git`.

## `const CONTENDER_TOOK: &str = "worktree lease contender: took the lease and gave it back";`

What the contender says when the lease was free.

## `const CONTENDER_REFUSED: &str = "worktree lease contender: refused: ";`

What it says, followed by the refusal, when it was not.

## `enum Contended {`

The contender's answer.

## `fn contend_for_the_worktree_lease(`

Ask another process for `repo`'s worktree lease, and say what it found. The process is this test
binary running [`worktree_lease_contender`] alone, `--ignored`, under the production `HostRunner`
with a 120-second bound — the runner owns, bounds and reaps it, as it does the husk producer's — and
it must run to its end (`1 passed`) and say one of its two lines, or the guard fails. A process of
its own, because the lease's two holds differ inside one process: the in-process claim refuses a
second acquisition here whatever the operating system lock is doing, and a POSIX `F_SETLK` lock
never excludes the process that holds it. Only another process sees whether the physical lock is
held.

## `fn assert_the_lease_is(held: bool, repo: &Path, git_dir: &Path, scratch: &Path, case: &str) {`

The physical premise, asked of another process. Free: the contender takes the lease and gives it
back. Held: it is refused as by another engine driving the worktree, and on Unix the refusal names
this process's pid — `F_GETLK`'s holder — so the lock is this test's and is still held. Windows'
`LockFileEx` reports no holder, so there the refusal alone is the answer.

## `fn worktree_lease_contender() {`

The contender: `WorktreeLock::acquire_in` on the named checkout and Git directory, then drops what it
took. Returns at once when its parent's variables are absent.

## `const DEFAULT_ROOT_HUSK_REPO: &str = "UPSTROKE_DEFAULT_ROOT_HUSK_REPO";`

The checkout holding the husk the default-root child resumes, named by its parent.

## `const DEFAULT_ROOT_HUSK_RUN: &str = "UPSTROKE_DEFAULT_ROOT_HUSK_RUN";`

The husk's id.

## `const DEFAULT_ROOT_HUSK_CREATION_ROOT: &str = "UPSTROKE_DEFAULT_ROOT_HUSK_CREATION_ROOT";`

The husk's creation private root, which the child snapshots.

## `const DEFAULT_ROOT_HUSK_DONE: &str =`

The line the child says once its last assertion has passed; its parent requires it, as
`DEFAULT_ROOT_UNREAD_DONE`'s does, since the child returns early, and still reports `1 passed`,
when its parent's variables are absent.

## `fn default_root_husk_resume_child() {`

The same physical treatment as the explicit-root guard above: `.git` outside the held interval,
the lease file accounted for before the first snapshot, and the contender — spawned from this child
— before the resume in each case and, held, after it.

The child of the test above. Its premises: the default root resolves under the child's home and
does not exist yet, the husk's creation root is outside that home, and the resolver refuses the
husk. With no default root, and then with one holding an include of other bytes in a directory the
writer would tighten, each with the lease free and held, the resume changes nothing under the home,
under the creation root, in the checkout or in `.git`, builds nothing, and is refused with the
resolver's own refusal, byte for byte.

Measured before commit under negative control A13: with no default root and the lease free, the
resume created the default root, its `git` directory and the include under the home ("created
.upstroke", "created .upstroke/git", "created .upstroke/git/recorded-objects.gitconfig"), and the
child and its parent fail.

## `fn recorded_execution_root(repo: &Path, run_id: &str) -> PathBuf {`

The execution root a schema-4 run's committed first line records, as text, for comparison with the
root a manager derives: both are the workspace manager's spelling, so neither side is a spelling
this test makes up.

## `fn execution_namespaces(repo: &Path) -> Vec<String> {`

The repository keys the fixture's private root holds execution roots under, sorted; empty before
any run made one.

## `fn manager_today(repo: &Path, run_id: &str) -> crate::workspace_manager::WorkspaceManager {`

`run_id`'s workspace manager as a resume derives it in this process: `WorkspaceManager::derive`
over the private root `RootDerived` reads from the run's record and the incarnation it recorded,
under this process's Git environment. Read-only.

## `type Places = Vec<(`

What [`common_dir_fixture`] returns: each place by name with its snapshot.

## `fn common_dir_fixture(repo: &Path, other: &Path) -> Places {`

Every place a resume under another common directory could write: the checkout outside `.git`
(`public_snapshot`: every run's log, report and lock files, the exclusion), the checkout's Git
directory whole (refs, packed refs, reflogs, objects, the worktree registry, the lease's file), the
private root (the include, every private half, every execution root and its intents, task, merge
and snapshot directories, every namespace), and the other common directory whole.

## `fn moved_in(before: &Places, after: &Places) -> Vec<String> {`

Every moved entry of every place, each prefixed with its place, so one assertion reports the whole
of what a resume wrote.

## `fn assert_admitted_at_the_recorded_roots(repo: &Path, open: &str, finished: &str) {`

The positive controls under the Git environment of the process that calls it: its manager of each
run derives the root that run recorded; the open run's resume is admitted and makes the exclusion
and the include from absent (`assert_admitted_and_published`); the finished run's resume is
finalized and refused as already finished, and finalization reclaims the scaffolding retained at the
root it recorded; and no other namespace appears under the private root.

## `const COMMON_DIR_REPO: &str = "UPSTROKE_COMMON_DIR_REPO";`

The checkout the common-directory children resume in, named by their parent.

## `const COMMON_DIR_OPEN_RUN: &str = "UPSTROKE_COMMON_DIR_OPEN_RUN";`

The parked run a child resumes.

## `const COMMON_DIR_FINISHED_RUN: &str = "UPSTROKE_COMMON_DIR_FINISHED_RUN";`

The finished run, its execution root retained, that a child resumes.

## `const COMMON_DIR_REFUSED: &str =`

The line the refusing child says once its last assertion has passed.

## `const COMMON_DIR_ADMITTED: &str =`

The line an admitting child says once its last assertion has passed.

## `fn common_dir_refused_resume_child() {`

The refusing child. Its premises: `GIT_COMMON_DIR` names another directory, absolute and outside the
checkout; the checkout's Git directory and its physical repository key are unchanged, and each run's
owner record names that key, so the owner and commit proofs still pass; and for each run, today's
manager takes the other common directory and derives another execution root than the run recorded.
For the open run and then the finished run, in each exclusion and include state, the resume changes
nothing in any place of [`common_dir_fixture`], builds nothing, and is refused naming the run, the
root it recorded, the root derived today, the canonical private root and the effective common
directory.

## `fn common_dir_admitted_resume_child() {`

An admitting child: with `GIT_COMMON_DIR` set by its parent to a spelling of the checkout's own
common directory, [`assert_admitted_at_the_recorded_roots`] for the open run its parent names and
the finished run.
