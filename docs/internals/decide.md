# Decide

[`src/decide.rs`](../../src/decide.rs) is `upstroke decide <run-id> [<question-id>]`: it asks a
run's recorded primary reviewer to propose an answer to each of the run's open questions, saves
each reply under `.upstroke/proposals/<run-id>/`, and answers nothing. A person applies a
proposal with `upstroke answer`.

## Module

The run is read through `engine::topology::observe`, the reading `answer` and `status` use, and
only a schema-4 run is accepted. The decider is `RunStarted4.reviews.primary` at
`RunStarted4.effort_policy.review`; a run with no primary binding is refused and nothing else is
chosen. Its profile is `review::profile_for`'s, which is read-only. Because that profile cannot
read `.upstroke/**`, and a decide call cannot read the run's private directory either, every
input the decider needs is handed to it in the prompt.

Admission comes before anything is written or any agent is asked, in two steps:

- Once per command, `admit` refuses a run recorded on the container runner, a run whose pools the
  credential pre-flight a resume runs (`preflight::refuse_unusable_profiles`) refuses, a reviewer
  whose adapter does not claim every `Guarantee`, a `--max-cost-usd` that is not a positive
  number, and a private directory no permission rule can name exactly.
- Once per call, `prepare` builds the command and refuses it unless it carries no environment
  overlay and `kept` finds every guarantee on the command and settings actually built. A claim
  is never enough on its own: the check reads the arguments and the settings, not the adapter's
  word.

Only Claude Code claims the guarantees (`guarantees_of`). Copilot and Codex report no dollar
cost, so a run whose primary reviewer is either is refused. What the decide call adds to an
ordinary reviewer's command is its own flags (`call_args`) and deny rules (`call_settings`), on a
profile built for the call; `review::profile_for`, `claude::build_args` and
`claude::permission_settings` are unchanged, so every ordinary reviewer and worker is given what
it was given before (`a_decide_call_adds_only_its_own_flags_to_an_ordinary_reviewers_command`).

What this module never does, and the reasons:

- It never answers. It calls no `interaction::write_answer`, no `AnswerSource` and no
  coordinator, and writes nothing under a run's public or private directory or an answers
  directory (`a_decide_leaves_every_file_of_the_run_as_it_was_and_writes_no_answer`). A proposal
  is advice for a person; applying it stays `upstroke answer`'s job, which checks the reply
  against the run as it is when the person applies it.
- No engine code calls it, and only the CLI names it
  (`only_the_cli_names_the_decider_and_no_engine_file_reaches_it`). `decide` is denied by path in
  `clippy.toml`, so a module that states no allowance cannot reach it.
- It asks no agent about a question that binds one (`OpenAsked::binds_an_agent`). Which agent
  runs a task is chosen only through an option the run froze, and an agent choosing agents is
  not a decision this command delegates.

Effects and their containment are stated in `effects/allowlist.toml`'s funnel row.

## `pub const INPUT_BOUND: usize = 64 * 1024;`

Each prompt input (the question, the task's text, its last failure, the earlier answers, the
plan file) is cut to this many bytes on a character boundary, and the prompt says where and of
how many bytes (`an_input_is_cut_on_a_character_boundary`). A plan can be large; one bound per
input keeps a long input from crowding out the others.

## `pub const MAX_PROPOSALS_PER_QUESTION: u32 = 2;`

A loop cap. A third `decide` on a question is refused before any agent is asked
(`a_third_proposal_is_refused_before_a_request_and_the_dollar_cap_stops_the_next_question`), so
re-running the command until an agent says what someone hoped for costs at most two replies.

## `pub const DEFAULT_MAX_COST_USD: f64 = 5.0;`

The default of `--max-cost-usd`. The costs this command's invocations report are summed, and once
the sum reaches the cap no further question is started; the report names the questions left. A
reply with no reported cost counts as unknown and is named, so the total is a floor. Each call is
also given what is left of the cap as its own ceiling (`call_args`), so a call that runs long
stops at the cap rather than after it.

## `pub struct DecideOptions {`

`pools_path` is `None` from the CLI, which reads the pools file `upstroke capacity` reads by
default; the tests name a pools file in their scratch tree. `environment` is `None` from the CLI,
so the credential pre-flight reads this process's environment through
`preflight::coordinator_environment`, as a run's does; the tests name a stand-in, so whether
`CLAUDE_CODE_OAUTH_TOKEN` is set where they run decides nothing.

## `enum Guarantee {`

The four promises every decide call keeps: its cost is reported in dollars, it stops at a dollar
ceiling of its own, it has no web access, and it cannot read the run's private directory. Each
is something `kept` can see on the built command and its settings.

## `fn guarantees_of(id: &str) -> &'static [Guarantee] {`

The explicit claim per adapter: all four for Claude Code, none for any other id, which `admit`
refuses. Copilot and Codex report no dollar cost (`Caps::cost_reporting` is false for both), so
neither could be summed against `--max-cost-usd` or stopped at a ceiling. A claim admits the
adapter only as far as `kept` then finds it true of the command
(`each_built_in_adapter_is_admitted_only_when_it_keeps_every_guarantee`).

## `fn call_args(ceiling_usd: f64) -> Vec<String> {`

The flags a decide call appends to the read-only profile's command, as `extra_args` of a profile
built for that call alone: `--max-budget-usd` with what is left of the cap, written as Rust's
shortest round-tripping decimal so `kept` reads back exactly the ceiling it was given, and
`--strict-mcp-config`, which with no `--mcp-config` starts no MCP server. Decide starts no process
before the call, so the CLI is not probed for either flag; a Claude Code too old to know one is
expected to reject it as an unknown option rather than run without it. No test here starts a
real CLI.

## `fn call_settings(profile: &WorkerProfile, private_denials: &[String]) -> Value {`

`claude::permission_settings` for the read-only profile with the private directory's deny rules
added to its deny list. The ordinary reviewer's settings are that function's output alone.

## `fn rule_path(dir: &Path) -> Option<String> {`

A directory in Claude Code's absolute rule spelling: `//`, then the path with `/` separators, a
Windows drive as its lower-case letter (`C:\Users` is `//c/Users`). A path that is relative, not
normalized, not Unicode, or that holds a pattern character (`* ? [ ] { } ( ) \`) or a control
character is `None`, because a rule written from it could name something else.

## `fn private_denials(private_dir: &str) -> Result<Vec<String>, String> {`

`Read(<dir>)` and `Read(<dir>/**)` for the run's recorded `private_dir`, and again for its
canonical path where that resolves to another one (a symlinked ancestor, macOS's `/private/var`).
The private directory is outside the repository by default (`~/.upstroke/runs/<run-id>`), so the
profile's relative `Read(.upstroke/**)` does not reach it, and Claude Code's `Read` tool can read
outside its working directory. Claude Code applies `Read` rules to Glob and Grep as well, and the
read-only profile allows no other tool.

## `fn admit(`

The once-per-command admission, in this order, each refusal saying that nothing was written and
no agent was asked:

1. A run recorded on the container runner (`RunStarted4.runner.kind`). Decide starts its
   reviewer on this host only, which is not where that run's reviewers ran
   (`a_container_run_is_refused_before_anything_is_written_or_asked`).
2. `preflight::refuse_unusable_profiles` over the pools file's pools and the recorded runner
   policy, the credential check a resume of the run makes, in its words
   (`a_run_the_credential_preflight_refuses_is_refused_before_anything_is_written_or_asked`).
3. An adapter that does not claim every `Guarantee`, naming the ones it lacks.
4. A `--max-cost-usd` that is zero, negative, infinite or not a number, which could be no call's
   ceiling and, as NaN, would never stop the loop
   (`a_cap_that_is_not_a_positive_number_of_dollars_is_refused_before_anything`).
5. A private directory `private_denials` cannot name
   (`a_private_directory_that_cannot_be_spelled_in_a_rule_is_refused_before_anything`).

## `pub struct Decided {`

The rendered report, and the ids of the questions whose reply was refused (or that were refused
before asking). `outcome` makes the command an error when any was refused; the CLI prints the
report first, so every question's line is shown before the command fails.

## `pub fn proposals_dir(repo_root: &Path, run_id: &str) -> PathBuf {`

`<repo>/.upstroke/proposals/<run-id>`. The name `proposals` appears in production code only here.

## `pub fn decide(`

Reads the run, chooses its questions and hands them to `propose`. The run is found by its exact
id; an unknown run, a schema 1-3 run and a run that cannot be read are refused with a message
saying which, before any agent is asked.

## `fn propose(`

One invocation per question through `runner::decide_request`, with
`InvocationId::Decide { ordinal }` numbered by the question's place in this command's order, the
adapter's built command with its stdin payload and settings file, and no environment overlay
(`the_request_is_a_decide_request_for_the_runs_reviewer_with_no_overlay`). `admit` runs first,
after the pools file is read and before anything else; each question's call is then built and
checked by `prepare`, with what is left of `--max-cost-usd` as its ceiling
(`each_calls_ceiling_is_what_is_left_of_the_cap`), before `ask` writes or starts anything for it.
The credential profile is the first pool of the reviewer's agent (`capacity::pool_for`), as a
review pass of the run would use; the timeout is the run's recorded review pass timeout, or the
review default. A question is not re-asked. Split from `decide` so a test can hand it a binding
question (`a_binding_question_is_skipped_and_no_request_is_made_for_it`).

## `fn prepare(`

Builds one call: the read-only profile with `call_args`, the settings `call_settings` gives, and
the adapter's command with its stdin payload and the settings file named. It writes and starts
nothing, and refuses the call unless the built command carries no environment overlay (naming the
overlay's keys and never their values) and `kept` holds for every `Guarantee`. The settings
path is the one the adapter's `materialize_permissions` used to choose,
`<id>.<n>-decide.json` beside the proposal. The first refusal comes before anything is written
for any question; the command's shape does not vary by question, so a later one would mean the
adapter changed between calls, and it still refuses that call before it is written or started
(`a_mocked_adapter_whose_command_breaks_a_promise_is_refused_before_anything`).

## `struct Built<'a> {`

A built command read flag by flag, beside its settings. `read` refuses a command that starts any
program but `claude`, carries a flag outside the decide call's own set (`BARE_FLAGS`,
`VALUED_FLAGS`), carries one twice (a second `--permission-mode` could override the one
checked), or ends on a flag with no value. So nothing the check does not
understand can reach the CLI: `--allowedTools`, `--add-dir`, `--resume`, `--mcp-config` or a
prompt argument is refused rather than passed over. `reads_only` requires the `--settings` value
and `CommandSpec::settings_file` to name the settings file written for the call, the settings to
hold an allow and a deny list and nothing else (no `defaultMode`, no hooks, no environment), and
the allow list to be `Read`, `Glob` and `Grep` at most.

## `fn kept(guarantee: Guarantee, built: &Built<'_>) -> Result<(), String> {`

What each guarantee requires of the built command:

- Cost reporting: `-p` and `--output-format json`, whose result carries `total_cost_usd`.
- A per-call dollar ceiling: `--max-budget-usd` with a finite value above zero and at most what
  is left of the cap.
- No web access: `--permission-mode dontAsk`, which denies every tool the settings do not allow;
  `--setting-sources ""`, so no user, project or local settings widen them; `--strict-mcp-config`;
  settings that allow reading only; and `WebFetch` and `WebSearch` denied by name.
- The run's private data unreadable: `--setting-sources ""`, settings that allow reading only, the
  profile's `Read(.upstroke/**)` and `Read(**/.upstroke/**)`, and every rule `private_denials`
  wrote.

## `fn ask(`

Creates the proposals directory, writes the call's settings with `util::write_json` (as
`materialize_permissions` wrote them before, replacing a settings file a killed earlier call
left), starts the call through the Runner, and saves its raw output.

## `fn chosen_questions(`

Without an id, every open question in the order raised (`Observed::raised_questions`). With one,
the open question of that exact id or of an unambiguous prefix within the run; a question the
run's log has settled is refused in `upstroke answer`'s words.

## `fn raised_by(event: &TopologyEvent) -> &'static str {`

The worker, a reviewer or integration, read from the event that opened the question. Where a
reviewer or integration raised it, the prompt says that they judged it a person's decision.

## `fn check_reply(question: &Question, text: &str) -> Result<Value, String> {`

Exactly one fenced `json` block, all six fields, their types, and a choice the run could apply as
a person's answer. A choice outside the options, a choice `interaction::answer_for_option` maps
to a decline (or whose text `interaction::interpret` reads as one), and `null` where the question
has options are refused (`a_bad_reply_is_refused_with_its_reason`,
`each_bad_reply_is_refused_saved_and_fails_the_command_after_the_others`).

## `fn next_number(dir: &Path, id: &str) -> Option<u32> {`

The next free proposal number from 1, or `None` at the cap.

## `fn write_new(path: &Path, bytes: &[u8]) -> Result<(), UpstrokeError> {`

Every proposal and raw output is created new (`File::create_new`) and never overwrites; a name
that is taken is an error rather than a replacement. The call's settings file is the one file
`ask` writes in place, and it holds no reply.
