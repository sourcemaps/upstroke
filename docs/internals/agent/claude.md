# `src/agent/claude.rs`

Extended notes for [`src/agent/claude.rs`](../../../src/agent/claude.rs).

[Source on GitHub](https://github.com/sourcemaps/upstroke/blob/master/src/agent/claude.rs).

The code defines current behavior. These notes preserve contracts and implementation
history. Search each backticked heading fragment separately in the source.

## Module

Claude Code adapter (DESIGN.md §16).

`claude -p` with the prompt on stdin, `--output-format json` parsed defensively, `--model`,
`--effort`, `--max-turns`, `--resume` for same-rung retries. This adapter never adds the skip-all
flag (a profile's `extra_args` are appended verbatim): [`permission_settings`] generates a narrow
per-run settings JSON the engine materializes to a file and this adapter passes via `--settings`,
keeping the workspace's own `.claude/settings.json` untouched.

## `#![allow(clippy::disallowed_methods, clippy::disallowed_macros)]`

LEGACY-EFFECT: this module is in the frozen legacy section of `effects/allowlist.toml`, which
carries its justification and the condition under which the section shrinks.

## `const PROBE_TIMEOUT: Duration = Duration::from_secs(60);`

Budget for one probe call. The CLI is a Node program behind an npm shim that runs an update check
on startup, so `--version` is slow on a loaded machine, and §19 makes a probe failure a refusal to
start: a slow machine timing out here loses the whole run rather than one attempt. Paid once per
run, at pre-flight.

## `mod probe_ordinal {`

Which of this adapter's pre-flight processes each identity is. A named table rather than a
counter, for the reason [`super::probe_request`] gives: an ordinal is a property of the step, so
two pre-flights of one machine mint the same identities whether or not an earlier step was
skipped. Dense from 0 and pairwise distinct, asserted by `every_preflight_process_has_its_own_ordinal`.

## `mod probe_ordinal` › `pub const ALL: [u32; 3] = [VERSION, HELP, AUTH_STATUS];`

Every ordinal above, for the uniqueness assertion.

## `fn probe(&self, runner: &dyn Runner) -> Result<Caps, UpstrokeError> {` › `let help = runner.run_blocking(&probe_request(`

Capabilities are read from `--help`, not assumed: this CLI has removed and hidden flags between
releases, and a missing flag must surface as a pre-flight refusal rather than as per-task failures
once a run is already spending (§16, §19).

## `fn probe(&self, runner: &dyn Runner) -> Result<Caps, UpstrokeError> {` › `read_only_mode: true,`

No single flag; achieved through the permission settings.

## `fn build(&self, run: &TaskRun) -> Result<CommandSpec, UpstrokeError> {` › `cli().spec(&build_args(run))`

No `current_dir`: the workspace is the runner's, carried on `RunnerRequest.workspace` (the runner
"owns cwd", `design/06_design_architecture.md`). No resolution either: `cli()` names the CLI and
the runner decides which file that is, so this and `probe` send one program string by
construction.

## `impl AgentAdapter for ClaudeCodeAdapter` › `fn discover`

`claude --setting-sources "" auth status --json`: a zero-spend auth probe that handles no token
and reads no credential file. The CLI answers about itself, and this reads its answer.

**It asks with no settings loaded, as a worker runs.** The first two arguments are
[`settings_isolation`]'s, the pair [`build_args`] gives every worker, so the CLI answers with no
user, project or local settings loaded. A settings file can supply a credential (its `env` can set
`ANTHROPIC_API_KEY`, and `apiKeyHelper` names a command that prints a key); workers load none of
those files, so a discovery that did would certify an account only a settings file signs in, and
every worker would meet it signed out. Pre-flight's refusal says the check loads no user, project
or local settings (`signed_out` in `src/engine/topology/preflight.rs`), because the operator's own
`claude auth status`, which loads them, can answer signed in.

No `--settings` is passed. A worker's `--settings` file is [`permission_settings`]' JSON, whose
only top-level key is `permissions`
(`permission_settings_carry_permissions_as_their_only_top_level_key`), so it supplies no
credential. `discovery_asks_with_exactly_the_settings_arguments_a_worker_is_given` compares this
request against `build_args`' own output, not a copy of it.

`connect`, the capacity report's agent status and pre-flight call this same `discover`, so they
report an account as a worker would meet it.

**What it still does not show.**

- That a credential works. `auth status` reports the credential the CLI has configured, and does
  not use it. An expired or invalid key, or a revoked login, answers signed in and passes, and the
  run meets it at its first attempt.
- Anything, from a CLI that rejects `--setting-sources` here. [`probe`] refuses a CLI whose
  `--help` does not advertise the flag ([`REQUIRED_FLAGS`]), but one that advertises it and still
  refuses it before `auth` exits with no JSON. [`parse_auth_status`] reads that as
  `AuthState::Unknown`, and pre-flight passes an unknown sign-in.
- A worker given more arguments than these. [`build_args`] appends a profile's `extra_args` after
  its own, and a worker given its own `--setting-sources` or `--settings` there could run in a
  settings context this discovery did not ask in. Nothing produces `extra_args` today: outside
  tests, the two places that construct a worker's profile leave it empty
  (`src/engine/assembly.rs:140`, `src/review.rs:466`). A producer for them would reopen the gap.

## `fn discover` › `discovery.notes.push(`

§13's tier classification comes from the catalog either way; saying so stops the pools file
reading as though the roster had been confirmed against this machine.

## `fn settings_isolation() -> [String; 2] {`

The two arguments that load no user, project or local settings: `--setting-sources` and an empty
list. They are the one source for both uses: [`build_args`] gives them to every worker, and
`discover` sends them before `auth status --json`, so a change here changes both
(`discovery_asks_with_exactly_the_settings_arguments_a_worker_is_given`).

## `fn effort_flag(effort: Effort) -> &'static str {`

Argument list, kept separate from binary resolution so it is testable on machines without the CLI
installed.

## `pub fn build_args(run: &TaskRun) -> Vec<String>` › `"--permission-mode".to_owned(),`

Anything not explicitly allowed is denied rather than prompted: an unattended run must never sit
waiting on a permission question.

## `pub fn build_args(run: &TaskRun) -> Vec<String>` › `args.extend(settings_isolation());`

Load no user/project/local settings: the per-run settings file is the whole permission surface.
Without this, allow rules from `~/.claude/settings.json` (or a repo's own `.claude/settings.json`)
union with ours and silently widen the sandbox (§20). The pair is [`settings_isolation`]'s, which
the sign-in discovery sends too.

## `pub fn permission_settings(profile: &WorkerProfile, gate_cmds: &[String]) -> Value {`

Narrow per-run permission settings (§20): edit profiles get file tools plus exactly the gate
commands; reviewers are read-only. Nobody gets network tools. The engine writes this JSON to the
run directory and the command carries it via `--settings`.

## `pub fn permission_settings(profile: &WorkerProfile, gate_cmds: &[String]) -> Value {` › `"deny": [`

No network tools, and no writing to the files that decide what later attempts may do: an agent
that can edit `.claude/` or `.git/` config escalates its own permissions for the rest of the run
(invariant 1 and §20).

`.upstroke/` is denied because `events.jsonl` is the source of truth: an agent that can append to
it could forge a `task_committed`, and one that can truncate it could erase its own failures.
Writes there are never legitimate; the engine owns that directory the way it owns git.

The `Read` denials are defence in depth rather than the mechanism. A gate runs repository code the
implementer just wrote, and that code can read any workspace path no permission rule ever sees.
The guarantee comes from §15's split: transcripts, verdicts, and settings live outside the
workspace, where there is no path to them at all.

## `fn parse_output(out: &ProcessOutput) -> Outcome {`

Defensive outcome parsing: the JSON result is trusted when present, but a missing or malformed
field never panics and never fails the parse — status degrades to `AgentError` instead. Diff,
transcript path, and pool drain are engine-owned and left empty here.

## `fn parse_output(out: &ProcessOutput) -> Outcome` › `outcome.detail = result_text;`

The agent's final message, not just error text: the reviewer's verdict travels in this field on
the success path, so leaving it None makes every review unparseable.

## `fn parse_output(out: &ProcessOutput) -> Outcome` › `let rate_limited = looks_rate_limited(&out.stderr)`

Rate-limit detection only applies to failures: a successful task about rate limiting ("added
backoff for 429 responses") must never be read as the pool being exhausted.

## `fn parse_output(out: &ProcessOutput) -> Outcome` › `outcome.detail = first_non_empty([`

Give the engine something to report: the CLI signals most failures through the JSON body with an
empty stderr.

## `fn parse_auth_status(out: &ProcessOutput) -> Discovery {`

Read `claude auth status --json` defensively: a missing or malformed field yields
[`AuthState::Unknown`], never an error and never a confident wrong answer.

The signed-out shape observed in Aug 2026 was `{"loggedIn": bool, "authMethod": "…",
"apiProvider": "…"}`; signed in (observed 2026-08-10, Max plan) it grew `email`, `orgId`,
`orgName` and `subscriptionType: "max"`. `loggedIn` drives the auth state; the rest distinguish
§13's two billing shapes, which decide the estimator rule the written pool gets. `subscriptionType`
classifies the shape even when `authMethod` matches neither token set, but it does not override the
other fields: when the three fields' tokens match both the API set and the subscription set,
`classify_shape` returns `None`.

## `fn parse_auth_status(out: &ProcessOutput) -> Discovery` › `return discovery.with_note(format!(`

A non-zero exit with no JSON is the shape an older CLI without the subcommand leaves. Not being
able to ask is not an answer, so it stays Unknown.

## `fn parse_auth_status(out: &ProcessOutput) -> Discovery` › `let subscription = payload`

Present only while signed in, and the one field that names the billing relationship outright: an
enterprise SSO whose `authMethod` matches no token still says `subscriptionType: "enterprise"`.

## `fn classify_shape`

§13's two billing shapes, from what the CLI says about the account.

Whole tokens against known sets, not substrings: `pro` sits inside `provider`, and a substring
test would read it there.

A wrong answer here is worse than none: `connect` prints "kind below is a default, not something
detected" only when this returns `None`, so a confident misclassification is written into the
pools file with the caveat suppressed. Anything ambiguous — a description matching both sets, or
neither — is therefore `None` on purpose.

## `fn classify_shape` › `_ => None,`

Both or neither: say so by saying nothing, and let the writer mark the pool's kind as the default
it is.

## `fn parse_usage(payload: &Value) -> Option<Usage>` › `reasoning_output_tokens: None,`

This CLI does not break output tokens down further, so the field stays empty rather than being
invented from a subtraction.

## `const CLI: &str = "claude";`

Binary discovery. On Windows the CLI may be a native `claude.exe` or an npm `claude.cmd` shim,
which CreateProcess cannot exec directly; the mechanics live in `super::bin`, shared with every
other adapter.

## `const CLI: &str = "claude";`

This CLI, as the boundary that will execute it names it. One name, not a platform-dependent
candidate list: choosing between `claude.exe`, `claude.cmd` and `claude.bat` is a filesystem
lookup this adapter does not perform. On Windows the extension search is `PATHEXT`'s and belongs
to whatever resolves the name, and inside a container image there is no extension to search.

## `const INSTALL_HINT: &str = "Install Claude Code there, or select a different agent.";`

What to tell an operator whose boundary has no `claude`.

## `fn no_profile_may_write_to_the_run_record()` › `for permissions in [PermissionMode::Edit, PermissionMode::ReadOnly] {`

The event log is the source of truth (invariant 4). An agent that could append to it could forge
a `task_committed`; one that could truncate it could erase its own failures. Neither a worker nor
a reviewer has a legitimate use for either.

## `fn no_profile_may_write_to_the_run_record()` › `assert!(deny.contains("Read(.upstroke/**)"), "{deny}");`

Defence in depth only; the enforceable half is §15's split, which puts transcripts outside the
workspace where no rule is needed.

## `fn successful_json_parses_to_completed()` › `assert_eq!(outcome.duration, out.duration);`

What the supervisor measured, carried through unchanged. Nothing downstream re-derives it: the
engine copies `Outcome.duration` into the attempt record and the report sums those, so an adapter
that dropped it would report every attempt as instantaneous.

## `fn a_successful_task_about_rate_limits_is_not_rate_limited()` › `let stdout = r#"{"type":"result","subtype":"success","is_error":false,`

The agent's own summary mentioning 429s must not be read as the pool being exhausted — that would
roll back verified work.

## `mod tests` › `fn a_failed_attempt_whose_result_says_you_hit_your_limit_is_rate_limited() {`

The shape Claude Code produced when an account reached its limit (2026-11-11): exit code 1,
`"is_error":true`, and `You've hit your 5-hour limit · resets 5:30am (UTC)` as `result`. It must
parse to `RateLimited`, which defers the task, rather than `AgentError`, which spends one of its
attempts, and the message must come through as the detail so the operator can see when the window
resets.

## `mod tests` › `fn you_hit_your_limit_on_stderr_is_rate_limited() {`

The same message on stderr, with nothing on stdout, also parses to `RateLimited`: `parse_output`
asks the vocabulary about stderr as well as the result text. The observed failure was the result
form; nobody has seen the stderr form from the CLI.

## `mod tests` › `fn a_failed_attempt_whose_result_says_you_hit_your_context_window_limit_is_an_agent_error() {`

A failed attempt in the five-hour one's shape whose result is
`You've hit your context-window limit; shorten the prompt`. A context window is not a quota window,
so it must parse to `AgentError`, which the ladder retries or escalates, and not to `RateLimited`,
which would defer a real failure and tell the capacity engine the pool is exhausted. The message
still comes through as the detail.

## `mod tests` › `fn a_successful_task_about_hitting_a_limit_is_not_rate_limited() {`

A successful attempt whose summary says "you've hit your 5-hour limit" stays `Completed`: the
vocabulary is only asked about failed attempts.

## `fn json_error_failures_carry_a_reportable_detail()` › `let stdout = r#"{"is_error":true,"subtype":"error_during_execution"}"#;`

Falls back to the subtype, then stderr, then a pointer.

## `mod tests` › `struct AuthStatusRecorder {`

A runner that records every request and answers each with a signed-in `auth status` payload, so
`discover` runs to an answer on a machine without the CLI, and the test reads what it asked.

## `mod tests` › `fn discovery_asks_with_exactly_the_settings_arguments_a_worker_is_given() {`

For a worker profile in each permission mode, with a per-run settings file: the flag
`--setting-sources` and the value after it are taken from `build_args`' own output, and
`discover`'s one request must begin with exactly those two arguments, then be
`auth status --json` and nothing else. The worker is given `--settings`, and the discovery is not.
Comparing against the worker's arguments rather than a literal copy is what makes this a check
that the two ask in one settings context.

## `mod tests` › `fn permission_settings_carry_permissions_as_their_only_top_level_key() {`

In both permission modes, the per-run settings JSON has one top-level key, `permissions`. That is
why the sign-in discovery can leave out the worker's `--settings`: the file carries no `env`, no
`apiKeyHelper`, nothing that could supply a credential. A key added here would need the discovery
to pass the file too.

## `mod tests` › `fn every_preflight_process_has_its_own_ordinal() {`

Each pre-flight process carries its own identity, and at one boundary this adapter runs 3 of them
(`--version`, `--help`, `auth status --json`), so the ordinals must be 3 distinct values. A run's
pre-flight runs `auth status --json` again for each further account its pools use, under ordinals
shifted by `PreflightIdentities::FURTHER_POOL_STRIDE` (`PreflightIdentities::further_pool`); this
test covers one boundary. The expected count is written from the steps the adapter performs, not
read from the table under test, which would agree with itself after losing an entry.

## `fn every_preflight_process_has_its_own_ordinal()` › `let ids: BTreeSet<String> = probe_ordinal::ALL`

And they render as 3 distinct probe-form identities naming this agent, which is the property the
ordinals exist for.

## `mod tests` › `fn probe_against_real_binary_when_present() {`

Runs only where the real CLI exists; skips elsewhere so CI without Claude Code stays green.

## `fn probe_against_real_binary_when_present()` › `if crate::util::find_program(CLI).is_none() {`

The host runner's boundary is this machine, so what gates this is whether this machine has the
CLI, asked of `util::find_program` rather than of the adapter, which does not know.

## `fn auth_status_reads_the_signed_in_shape_including_the_plan` › `let signed_in = output(`

The field set observed on a signed-in machine (2026-08-10, Max plan), identifiers dummied.
`subscriptionType` must classify the shape even if `authMethod` were something no token matches.

## `fn auth_status_reads_the_signed_in_shape_including_the_plan` › `let sso = output(`

The same payload with an unrecognized auth method still classifies, because the method matches
neither token set and `subscriptionType` matches the subscription set.

## `fn auth_status_reads_the_signed_in_shape_including_the_plan` › `let signed_out = output(`

Signed out (observed the same day): no `subscriptionType`, nothing conclusive, so the shape is
None.
