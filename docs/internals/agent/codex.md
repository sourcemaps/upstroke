# `src/agent/codex.rs`

Extended notes for [`src/agent/codex.rs`](../../../src/agent/codex.rs).

[Source on GitHub](https://github.com/sourcemaps/upstroke/blob/master/src/agent/codex.rs).

The code defines current behavior. These notes preserve contracts and implementation
history. Search each backticked heading fragment separately in the source.

## Module

OpenAI Codex CLI adapter (DESIGN.md §16): a second pool, and a reviewer from a different model
family that costs nothing on the Claude window.

**Implementing works where the sandbox is real, and only there.** This CLI's sandbox is an
external helper, present on Linux and absent on Windows, so on Windows `exec` silently degrades
to read-only and [`refuse_edit_profile`] turns an implementer away at build time rather than
letting it spend attempts on empty diffs. On Linux the same flags write inside the workspace and
are blocked outside it, which is what §20 asks for. The evidence is under `edit_refusal`.

The judge's seat works everywhere: `read-only` is enforced on every platform, and the family is
genuinely different from Anthropic's (§11.3).

**Two command shapes, not one with a flag swapped.** `codex exec` and `codex exec resume` accept
*different* flag sets: resume takes no `-s`, no `-C`, no `--profile`. The sandbox is a property
of the session, fixed when it is created and inherited by every resumed turn, which is
upstroke's model: a same-rung retry has the same profile by definition (§11.4). Observed against
codex-cli 0.147.0: a resume with no sandbox flag ran under the policy its session recorded.

**The prompt goes on stdin, as `-`.** Windows caps a command line at ~8,191 characters and a
review prompt carries up to [`crate::review::MAX_DIFF_BYTES`] of diff, so argv was never an
option. The CLI also *waits* on stdin when it expects input ("Reading additional input from
stdin…"), so the payload must always be written and the pipe always closed; [`super::proc`] does
both.

**stdout is JSONL, stderr is tracing.** `--json` emits one event per line (`thread.started`,
`turn.started`, `item.started`, `item.completed`, `turn.completed`) while stderr carries
`ERROR codex_api::…` log lines. Events are parsed from stdout only; on a failed exit stderr is
also checked for a rate limit and is the last fallback for `detail`.

**Tokens, not dollars.** The route reports a resumable session id (`thread_id`), the final
message and token usage, but no price. Tokens are recorded and `cost_reporting` stays false, so
the ledger says `?` rather than inventing a price from a rate table inside a published binary
that would go stale silently, for a figure that is notional on subscription auth. §13: an
estimate that flatters is worse than none.

**Two of this CLI's own features are deliberately unused for model turns.**

`codex review` would swap the standard. §11.3's second opinion is *the same standard, a
different judge*: upstroke's review prompt carries the acceptance criteria, the anti-sycophancy
framing, the `DATA UNDER REVIEW` fencing and the operator's decisions (§12). A verdict from
OpenAI's rubric on a bare diff is not comparable with the Claude reviewer's, and a cross-family
disagreement would be uninterpretable. Reviews therefore run through plain `exec` with
`--sandbox read-only`. The adapter has no review mode; a reviewer reaches it as
[`PermissionMode::ReadOnly`].

`--output-schema` on a model turn would make a third copy of the verdict shape (prompt, parser,
schema) that can drift, hold two reviewers to two contracts, and push the reviewer's prose into
escaped strings. The re-ask-on-unparseable path already covers that failure. Pre-flight does pass
a deliberately missing schema path to the CLI's local parser; that is a zero-spend guard proving
the exact reasoning key before any model turn, not an output contract.

**Never added by this adapter:** `--dangerously-bypass-approvals-and-sandbox`,
`--dangerously-bypass-hook-trust`, `-s danger-full-access` (§20 grants narrow permissions; its one
scoped exception, `external-sandbox` under a container runner, is not built here), and `--ephemeral`, which would discard the session §11.4's same-rung
retry resumes. `build_args` does append the profile's `extra_args` verbatim.

Surface captured from `codex --help`, `codex exec --help` and `codex exec resume --help` at
0.147.0, and verified by running it.

## `#![allow(clippy::disallowed_methods, clippy::disallowed_macros)]`

LEGACY-EFFECT: this module is in the frozen legacy section of `effects/allowlist.toml`, which
carries its justification and the condition under which the section shrinks.

## `const PROBE_TIMEOUT: Duration = Duration::from_secs(60);`

Budget for one probe call. Generous because §19 makes a probe failure a refusal to start, so a slow
machine timing out here loses a whole run rather than one attempt. Paid once per run.

## `const CONFIG_PROBE_UNKNOWN_KEY: &str = "upstroke_probe_deliberately_unknown";`

The strict-config control must be rejected before the missing-schema guard. If it is not, the CLI
has stopped enforcing the parser contract this probe relies on, and an apparently successful
effort-key check would mean nothing.

## `const REQUIRED_EXEC_FLAGS: [&str; 5] = ["--json", "--sandbox", "--model", "-c", "--config"];`

Flags `exec` must still advertise at pre-flight. Without `--json` there is no session id and no
usage; without `--sandbox` a reviewer could edit the code it is judging. A CLI that dropped one
must refuse the run up front, not fail attempts once spending (§19).

## `mod probe_ordinal {`

Which of this adapter's pre-flight processes each identity is, named rather than counted for the
reason [`super::probe_request`] gives. The adapter names its CLI and the boundary resolves it, so
every process it starts is a fixed, named step; that the table is the whole domain at one boundary
is what `preflight_starts_exactly_the_processes_the_ordinal_table_declares` asserts.

## `mod probe_ordinal` › `pub const CONFIG_BASE: u32 = 3;`

The six strict-config parser probes: two surfaces x {unknown-key control, xhigh, max}, at
`CONFIG_BASE + surface * CONFIG_PER_SURFACE + step`.

## `mod probe_ordinal` › `pub const ALL: [u32; 12] = [`

Every fixed ordinal above, for the uniqueness assertion.

## `fn probe(&self, runner: &dyn Runner) -> Result<Caps, UpstrokeError> {` › `let fresh_help = runner.run_blocking(&probe_request(`

Fresh and resumed attempts are different CLI surfaces. Both carry the reasoning override, so both
must prove `--config` before spend; only fresh attempts carry the sandbox.

## `fn probe(&self, runner: &dyn Runner) -> Result<Caps, UpstrokeError> {` › `let models = runner.run_blocking(&probe_request(`

The strict local parser proves the exact key and the two role policy values. The local catalog is
separate zero-spend evidence for each model × effort pair, so every known Codex model must expose
every shared effort level before a run can start.

## `fn probe(&self, runner: &dyn Runner) -> Result<Caps, UpstrokeError> {` › `json_output: true,`

Asked for and parsed, unlike Copilot's route, where the flag would promise an envelope no caller
reads.

## `fn probe(&self, runner: &dyn Runner) -> Result<Caps, UpstrokeError> {` › `session_resume: true,`

`codex exec resume <id>`, observed to round-trip: the resumed turn returned the same `thread_id`
and recalled the prior exchange.

## `fn probe(&self, runner: &dyn Runner) -> Result<Caps, UpstrokeError> {` › `cost_reporting: false,`

Tokens, not dollars (see the module header): a decision about what upstroke will claim, not a
missing feature.

## `fn probe(&self, runner: &dyn Runner) -> Result<Caps, UpstrokeError> {` › `acp: false,`

The CLI has `mcp-server` and `app-server`, neither of which is ACP, and this adapter spawns a
process per attempt either way.

## `fn probe(&self, runner: &dyn Runner) -> Result<Caps, UpstrokeError> {` › `model_list: true,`

`debug models` is a local catalog rather than a network query; probe validates it and discovery
exposes its slugs.

## `fn build(&self, run: &TaskRun) -> Result<CommandSpec, UpstrokeError> {` › `cli().spec(&build_args(run))`

The working root comes from the runner's cwd (`RunnerRequest.workspace`, the split in §6), not
from `-C`: `exec resume` has no `-C`, and one mechanism for both shapes beats two that must agree.

`cli()` names the CLI and the runner decides which file that is, so `build` performs no lookup
and sends exactly the program string `probe` certified.

## `impl AgentAdapter for CodexAdapter` › `fn discover`

This CLI answers "am I signed in?" without spending anything. `codex login status` is
non-interactive, exits 0 either way, and prints `Logged in using ChatGPT` or `Not logged in`
(observed). Copilot's adapter reports [`AuthState::Unknown`] because GitHub documents no such
query; here `upstroke connect` can write a pool an operator can trust.

## `impl AgentAdapter for CodexAdapter` › `fn materialize_permissions(`

Permissions are argv here, as for Copilot, but the audit file is still written: §15 calls
`settings/<task>-<attempt>.json` the per-attempt permission surface, and a trail that exists for
one agent and silently not another is worse than none.

## `impl ConfigProbeSurface` › `const fn index(self) -> u32 {`

Which surface this is, so its three parser probes get their own block of invocation ordinals.

## `struct MissingOutputSchema {`

A unique empty directory whose child path is guaranteed not to exist. Codex validates
`--output-schema` locally before starting a turn, making that absent child a deterministic,
zero-spend stopping point.

## `fn drop(&mut self)` › `let _ = std::fs::remove_dir(&self.dir);`

The child is never created. No recursive cleanup: if a surprising CLI did write anything,
preserving it is safer than deleting an unexpected artifact.

## `for (step, effort) in [Effort::XHigh, Effort::Max].into_iter().enumerate() {`

The two policy values upstroke promises for the roles this feature introduced. The model catalog
validates the remaining shared values; accepting either assignment proves the exact key, and
checking both catches a provider-side enum regression.

## `fn effort_flag(effort: Effort) -> &'static str {`

This CLI's name for a tier-neutral effort level: a function rather than a `Display` impl, so a
vendor can differ without the engine learning about it. Every value is in the provider's validated
enum (`low, medium, high, xhigh, max` plus `none` and `minimal`), as checked against the 400 it
returns for anything else.

## `fn sandbox_mode(profile: &WorkerProfile) -> &'static str {`

The sandbox this profile runs under (§20). Two modes and no third: `danger-full-access` exists and
is never used. A reviewer may read and nothing else, because a reviewer that edits the code it is
judging has invalidated its own verdict.

## `fn edit_refusal(profile: &WorkerProfile) -> Option<UpstrokeError> {`

Why an implementer is refused **on Windows only**, and what was measured.

The sandbox is an external helper. `codex doctor` reports `linux helper: <path>` where one exists
and `none` on Windows, where nothing can enforce a boundary. The binary states the consequence:

> `approval_policy = "never"` cannot be used because requirements do not
> allow `sandbox_mode = "danger-full-access"`; Codex would fall back to
> read-only permissions with approvals disabled.

`exec` is non-interactive, so it forces `never`. With no enforceable sandbox that degrades to
read-only, and `--sandbox workspace-write` is *accepted and then ignored*: exit 0, no warning, no
diff. A measured run spent both attempts on empty diffs and parked asking for write access it had
been granted. `-c approval_policy="on-request"` and `-c permission_profile="…"` were both tried;
`exec` wins.

The only mode that writes there is `--approve-for-me`, which routes approvals through an automatic
reviewer and is not a sandbox: asked to write outside the repository it did so, and
`sandbox_workspace_write.writable_roots` did not constrain it. §20 grants permission by
mechanism, and §14's rollback is `git clean -fd` *inside* the workspace, so anything written
outside it survives a failed attempt.

**On Linux the sandbox is real.** Same CLI, same flags, helper present: `--sandbox
workspace-write` wrote inside the workspace and was blocked outside it, both measured. So the
refusal is scoped to the platform that cannot enforce it.

For whoever containerises this: Docker's default seccomp profile blocks the syscalls the sandbox
needs to initialise, with a *different* message ("the workspace sandbox failed to initialize") and
the same empty-diff result. `--security-opt seccomp=unconfined --cap-add SYS_ADMIN` let it
initialise; which of the two is strictly required was not isolated.

`build` calls this before it builds any argv.

## `pub fn build_args(run: &TaskRun) -> Vec<String> {`

The argument list `build` hands to `cli().spec`; tests call it directly.

Two shapes, because the CLI has two. A fresh attempt sets the sandbox the session will carry; a
resumed one inherits it and would be rejected for passing `-s` at all (observed: exit 2,
"unexpected argument '-s' found").

## `pub fn build_args(run: &TaskRun) -> Vec<String>` › `args.push("--model".to_owned());`

Passed on both shapes: the recorded command says what it ran on without opening the session
file, and a change to the CLI's default must not silently move a resumed retry to another model.

## `pub fn build_args(run: &TaskRun) -> Vec<String>` › `if let Some(effort) = run.profile.effort {`

Effort, for the same reason as the model. This CLI's default comes from the *provider's* roster,
not the flag set (`gpt-5.6-sol` was seen carrying `default_reasoning_level: low`), and a roster
refresh could move it without a release. Passed on the resumed shape too: `-c` is accepted there
(measured, unlike `-s`), and a retry must not think harder or less hard than the attempt it
continues.

## `pub fn build_args(run: &TaskRun) -> Vec<String>` › `args.push("-".to_owned());`

`-` is "read the prompt from stdin" and must be last: everything after it would be taken as the
prompt's own arguments.

## `fn parse_output(out: &ProcessOutput) -> Outcome {`

Outcome parsing over the JSONL event stream. A line that is not JSON is skipped rather than
failing the attempt, and a missing field leaves its slot empty instead of panicking. The engine
owns `diff` and `transcript_path`.

## `fn parse_output(out: &ProcessOutput) -> Outcome` › `message = item`

Last one wins: the final message is the agent's answer, and the field a reviewer's verdict
travels in.

## `fn parse_output(out: &ProcessOutput) -> Outcome` › `usage = Some(add_usage(usage, event.get("usage")));`

Summed rather than replaced. One invocation was measured emitting exactly one of these, tool
call and all, so this is defence against a version that reports per step, where taking the last
would quietly under-count.

## `fn parse_output(out: &ProcessOutput) -> Outcome` › `let joined = errors.join("\n");`

Failures only: a successful task *about* rate limiting must never read as the pool being
exhausted (see `looks_rate_limited`).

## `fn parse_output(out: &ProcessOutput) -> Outcome` › `outcome.detail = [`

The `error` events first: stderr is a tracing log here, so the event stream carries the
diagnostic a human wants. An unauthenticated run exits 101 with 401s, which is an agent error and
not a rate limit, a distinction the ladder acts on.

## `fn add_usage(total: Option<Usage>, reported: Option<&Value>) -> Usage {`

Fold one `turn.completed`'s usage into the running total. `reasoning_output_tokens` is a *subset*
of `output_tokens` on this CLI, so it is carried in its own slot rather than added to output;
summing both would double-count the thinking.

## `fn add_usage` › `add(`

Vendor names differ; the concepts line up. `cached_input_tokens` is a read from the cache,
`cache_write_input_tokens` a write into it.

## `fn add_usage` › `total.num_turns = Some(total.num_turns.unwrap_or(0) + 1);`

One `turn.completed` that carries `usage` is one turn: `add_usage` returns before counting when
`usage` is absent, so such an event adds no turn.

## `fn parse_login_status(out: &ProcessOutput) -> Discovery {`

Observed forms (0.147.0): `Not logged in` and `Logged in using ChatGPT`. The negative is checked
first because it contains the positive as a substring; matching "logged in" first would call a
signed-out account signed in, which is the one error `AuthState` exists to prevent.

## `fn parse_login_status(out: &ProcessOutput) -> Discovery` › `if text.contains("chatgpt") {`

§13's two billing shapes. Only a ChatGPT plan sets the shape, to a rate-limit window. An API key
leaves `shape` unset with a note that the pool kind is a default; any other signed-in text leaves
it unset too, for the caller's documented default rather than a guess.

## `const CLI: &str = "codex";`

The CLI this adapter names, and what to tell an operator whose boundary does not have it.
`super::bin` owns the mechanics.

## `const CLI: &str = "codex";`

One name, not a platform-dependent candidate list. Which file the name is belongs to the boundary
that executes it, which with a container runner is not this machine's filesystem. Applying `PATHEXT`
is the resolver's job. The host runner's resolver stats candidates and spawns none, so it does not
skip one that refuses to spawn (such as a Windows Store package payload).

## `const INSTALL_HINT`

What to tell an operator whose boundary has no `codex`.

## `impl AdapterSource for CodexAdapter {`

A source holding this one adapter: `get` returns it for its own id and `None` for any other. `by_id`
does not use it; it scans `ADAPTERS`.

## `mod tests` › `const FORBIDDEN: [&str; 4] = [`

Flags `build_args` must never emit from its own arguments: three would hand the agent the machine
(§20), and `--ephemeral` would discard the session a same-rung retry resumes. The fixture's
`extra_args` is empty, and `build_args` appends a profile's `extra_args` verbatim, so what they
hold is not checked here.

## `mod tests` › `fn debug_models_json(levels: &[&str]) -> String {`

Advertises every Codex model the catalog knows, not one hard-coded slug: the probe checks them
all, so a complete catalog must list them all.

## `fn model_catalog_requires_every_effort_for_each_known_codex_model()` › `for slug in &known {`

Every catalogued slug gets its own refusals: missing, listed only under a wrong spelling, and short
of each required effort in turn. So a local catalog that predates `gpt-6-astra` is refused by name
before spend, even for a run that only pins `gpt-5.6-sol`.

## `fn a_fresh_attempt_sets_its_sandbox_and_a_resumed_one_must_not` › `let fresh = build_args(&run(PermissionMode::Edit, None));`

`exec resume` rejects `-s` (observed: exit 2) because the sandbox belongs to the session. Passing
it anyway would fail every same-rung retry for a reason unrelated to the code.

## `fn a_profile_without_an_effort_passes_none_rather_than_guessing` › `let mut run = run(PermissionMode::Edit, None);`

Passing a guess would be worse than the CLI's own default, because it would look deliberate.

## `fn the_prompt_is_the_last_argument_and_it_is_stdin()` › `for resume in [None, Some("sess")] {`

The prompt carries the diff and Windows caps argv at ~8,191 characters, so it goes on stdin via
`-`, last, on both shapes.

## `fn the_prompt_is_the_last_argument_and_it_is_stdin()` › `let run = run(PermissionMode::Edit, None);`

And the stdin payload is the prompt itself: the CLI waits on stdin until it is written and closed.

## `fn an_implementer_is_refused_where_no_sandbox_can_enforce_it` › `let err = edit_refusal(&profile(PermissionMode::Edit))`

Windows has no sandbox helper, so `exec` would write nothing while returning 0 (see
`edit_refusal`). A capability this platform cannot deliver is a refusal to start (§19), not a task
that fails after spending.

## `fn an_implementer_is_refused_where_no_sandbox_can_enforce_it` › `assert!(text.contains("Linux"), "{text}");`

And where to go instead: Linux, or another agent.

## `fn an_implementer_is_allowed_where_the_sandbox_is_real()` › `assert!(`

The refusal is scoped to the platform that cannot enforce a boundary, not to the CLI.

## `fn a_reviewer_is_read_only_and_nothing_is_ever_given_the_machine` › `assert!(edit_refusal(&profile(PermissionMode::ReadOnly)).is_none());`

Never refused anywhere: read-only is enforced on every platform, and it is the seat this adapter
is most useful in.

## `fn a_successful_run_yields_its_session_message_and_tokens()` › `let stdout = r#"{"type":"thread.started","thread_id":"019ff122-4d61-7323-a217-843ddfe5932c"}`

A real event stream, from a tool-using run against codex-cli 0.147.0.

## `fn a_successful_run_yields_its_session_message_and_tokens()` › `assert_eq!(outcome.duration, out.duration);`

What the supervisor measured, carried through unchanged, as the Claude adapter asserts too.

## `fn a_successful_run_yields_its_session_message_and_tokens()` › `assert_eq!(outcome.detail.as_deref(), Some("hi"));`

The agent's final message, not the `command_execution` item before it.

## `fn a_successful_run_yields_its_session_message_and_tokens()` › `assert_eq!(outcome.cost_usd, None);`

Tokens, never a price.

## `fn several_turns_are_summed_rather_than_last_wins()` › `let stdout = r#"{"type":"turn.completed","usage":{"input_tokens":10,"output_tokens":2,"reasoning_output_tokens":1}}`

The guard for a version that reports per step, where taking the last would under-count the run.

## `fn several_turns_are_summed_rather_than_last_wins()` › `assert_eq!(usage.reasoning_output_tokens, Some(5));`

Carried, not double-counted: reasoning tokens are a subset of output.

## `fn an_unauthenticated_run_is_an_agent_error_not_an_exhausted_pool` › `let stdout = r#"{"type":"thread.started","thread_id":"t1"}`

Observed: five 401 retries then exit 101. A rate limit defers and waits for a window, an agent
error spends an attempt, so calling a signed-out account "rate limited" would park a run forever
on a problem that never resolves.

## `fn junk_on_stdout_never_fails_an_attempt()` › `let stdout = "Reading additional input from stdin...\n\`

Warnings, progress chatter, a half-written line at a kill: none of it is JSON, and none of it
should turn a finished attempt into a failure.

## `fn signed_out_is_never_read_as_signed_in()` › `let signed_out = parse_login_status(&output(0, "Not logged in\n", ""));`

"Not logged in" contains "logged in", so the order of checks is the test: a confident wrong
"signed in" writes a pool the operator trusts and a run then fails against.

## `fn signed_out_is_never_read_as_signed_in()` › `let odd = parse_login_status(&output(0, "something new entirely\n", ""));`

Anything unrecognised stays Unknown and says so in a note.

## `mod tests` › `struct RecordingRunner {`

A boundary that answers each of the twelve processes this adapter's `probe` and `discover` start
(ten and two) the way a working `codex` does, and records each request. The answers let the
sequence *complete*: a validator that refuses stops the walk, and a walk that stops after one
process says nothing about the identities of the rest.

It answers by **argument**, never by program: what the CLI is called at the boundary is the
boundary's business, and a fixture keyed on the program string would assert the adapter's answer
against itself.

## `impl crate::runner::contract::tests::InlineRunner for RecordingRunner` › `return Ok(output(`

The control: the strict parser rejects the unknown key *before* the local missing-schema guard.

## `impl crate::runner::contract::tests::InlineRunner for RecordingRunner` › `return Ok(output(`

The key is accepted, and the run then stops on the schema file that deliberately does not exist.

## `impl crate::runner::contract::tests::InlineRunner for RecordingRunner` › `let models: Vec<_> = catalog::known_models(ADAPTER_ID)`

Every model the catalog knows, each advertising every effort, derived from the catalog: nothing
here asserts *which* models exist.

## `mod tests` › `fn the_six_config_parser_probes_are_six_distinct_identities() {`

The six strict-config parser probes are six identities. The retired design packet's
`decisions.admission_and_leases.permits.invocation_identity`, which is not in this repository,
made `InvocationId` unique per process; two processes sharing one collide in the invocation
ledger and in invocation-derived containment scopes.

`every_preflight_process_has_its_own_ordinal` checks the hand-written table, which holds only
declared ordinals. These six are computed (`CONFIG_BASE + surface.index() * CONFIG_PER_SURFACE +
step`); a `ConfigProbeSurface::Resume` whose `index()` returned `Fresh`'s once left them carrying
three identities with the suite green. So this test asks the requests, not the table.

The invocation is built with [`Invocation::at`] over an absolute program, independent of what
this machine has installed.

## `fn the_six_config_parser_probes_are_six_distinct_identities` › `assert!(`

And they are the probe form naming this agent, so a "distinct" set cannot be six values of some
other shape.

## `fn the_six_config_parser_probes_are_six_distinct_identities` › `let resumed = runner`

Three of the six carry `resume`, so both surfaces were probed, not one surface six times.

## `fn the_six_config_parser_probes_are_six_distinct_identities` › `let declared: BTreeSet<u32> = probe_ordinal::ALL.into_iter().collect();`

Each computed ordinal must be one the table reserves.

## `mod tests` › `fn preflight_starts_exactly_the_processes_the_ordinal_table_declares() {`

The twelve declared ordinals are **exactly** the processes this adapter's `probe` and `discover`
start at one boundary, ten and two: no process takes an identity the table does not enumerate. A
run's pre-flight calls only `probe` for this agent, since its sign-in check covers Claude Code alone
(`SIGN_IN_CHECKED` in `engine::topology::preflight`).

## `fn preflight_starts_exactly_the_processes_the_ordinal_table_declares` › `let declared: BTreeSet<u32> = probe_ordinal::ALL.into_iter().collect();`

The ordinals used must equal the ordinals declared: a step taking an ordinal nobody reserved, or a
reserved ordinal no step uses, fails here.

## `mod tests` › `fn every_preflight_process_names_the_cli_at_whichever_boundary_is_asked() {`

Each of the twelve processes `probe` and `discover` start names the CLI and nothing this machine
contributed; a run's pre-flight starts only `probe`'s ten.

The same adapter is then driven against a second boundary in the same process, and that boundary
must be asked the identical program strings. A resolution memoised in a process-wide cell is
invisible to a test that constructs one runner, and would hand the second boundary the first one's
answer.

## `fn every_preflight_process_names_the_cli_at_whichever_boundary_is_asked` › `let programs = first.programs();`

`codex`, written here rather than read from `CLI`: a constant compared against itself proves
nothing.

## `mod tests` › `fn every_preflight_process_has_its_own_ordinal() {`

This adapter's `probe` and `discover` start 12 processes (a run's pre-flight starts `probe`'s ten),
so the ordinals it fixes must be 12 distinct values. The expected count is written from the steps the adapter performs, not read from the table under
test, which would otherwise agree with itself after losing an entry.

## `fn every_preflight_process_has_its_own_ordinal()` › `let ids: BTreeSet<String> = probe_ordinal::ALL`

And they render as 12 distinct identities in the probe form naming this agent
(`p.agent-codex.o…`), which is the property the ordinals exist for.

## `mod tests` › `fn probe_against_real_binary_when_present() {`

Runs only where the real CLI exists, and returns early elsewhere. The deterministic fixtures replay
the CLI behaviour this adapter was written against and do not detect a CLI that has since changed;
where this test runs, it catches local help or catalog drift.

## `fn probe_against_real_binary_when_present()` › `if crate::util::find_program(CLI).is_none() {`

The host runner's boundary *is* this machine, so what gates this is whether this machine has the
CLI, asked of `util::find_program` rather than of the adapter, which does not know.
