# Decide

[`src/decide.rs`](../../src/decide.rs) is `upstroke decide <run-id> <question-id>`: it asks a
parked run's recorded primary reviewer to propose an answer to one open question, saves the reply
in its claim of the run, `decide/` in the run's private directory, and answers nothing. A person
applies a proposal with `upstroke answer`; the report names the proposal's path.

One invocation decides exactly one question of one run, with one call. The question id is
required and matched exactly, byte for byte: no case is folded and no prefix matched
(`chosen_question`). The run id is checked as a plain file-name component before any path is
built from it (`decide`). The command takes the run's own lock, the `rundir::RunLock` `resume`
takes, before the run is read or admitted, and holds it until the proposal is written or the
command has failed, so no resume can start while the reviewer is asked and nothing is asked of a
run a coordinator holds. Under the lock it requires the run to be parked as the fold of its log
reports it (`parked`), and it reads whether the question has an answer at admission and again
immediately before the call (`unanswered`).

A run is decided once. The command claims it by creating `decide/` (`CLAIM_DIR`) in the run's
private directory exclusively, immediately before its call, and never reads that directory or
removes it: a run whose claim exists, complete, partial or interrupted, or anything else at that
name, is refused, never adopted, with nothing written and no agent asked (`Claim::take`). The
private directory is the one `run_started` records, outside the checkout, so every copy of the
checkout names the same one and shares the claim: each copy has a run lock of its own, and it is
the claim that a second command from either copy finds
(`copied_checkout_shares_the_run_allowance`,
`the_exclusive_create_dir_is_what_makes_a_second_command_find_the_claim`). That directory is the
run's one spending claim: the one call decide makes for the run is made by the command that
created it, so a later decide, of the same question or another, from any copy, or one started
beside it that loses the `create_dir`, spends nothing. The claim holds three files whose names
are fixed (`SETTINGS_FILE`, `RAW_FILE`, `PROPOSAL_FILE`), so no question id becomes a file name
(`no_question_id_becomes_a_file_name`). The call is capped at `--max-cost-usd`, the run's allowance, whole; nothing is
subtracted from it and no other call follows, so there is no remainder to compute. Only a clean,
successful, priced call is settled, at its price (`settle`); any other outcome is saved as
refused, and a refused proposal makes the command exit non-zero (`Decided::outcome`).

## Module

The run is read through `engine::topology::observe`, the reading `answer` and `status` use, and
only a schema-4 run is accepted. The decider is `RunStarted4.reviews.primary` at
`RunStarted4.effort_policy.review`; a run with no primary binding is refused and nothing else is
chosen. Its profile is `review::profile_for`'s, which is read-only. Because that profile cannot
read `.upstroke/**`, and a decide call cannot read the run's private directory either, every
input the decider needs is handed to it in the prompt.

Admission comes under the run's lock and before the run is claimed, anything is written or any
agent is asked, in these steps:

- `decide` refuses a run id that is not a plain component, a run with no log, a run whose lock
  another holder has (a coordinator, a resume, another decide, or a reaper still cleaning up),
  a run that is not schema 4, a run the fold does not report parked (`parked`), and a question
  id that names no open question exactly (`chosen_question`).
- `propose` refuses a question that binds an agent. A question id is never a file name, so it
  is not checked as one.
- `admit` refuses a run recorded on the container runner, a run whose pools the credential
  pre-flight a resume runs (`preflight::refuse_unusable_profiles`) refuses, a reviewer whose
  adapter does not claim every `Guarantee`, a `--max-cost-usd` that is not a positive number, a
  private directory `closed_private_dir` does not admit (the claim relies on it), and a private
  directory no permission rule can name exactly.
- `unanswered` reads the question's answer file (`interaction::read_answer`), so a question a
  person has answered already is refused and never asked.
- The recorded reviewer passes the pre-flight a run makes of each recorded agent
  (`preflight::certify_agent`, which `RunPreflight::certify` runs too): its CLI probed through
  the Runner behind the wrapper a resume installs over the runner its pre-flight probes through
  (`profiled::profiled_borrowing`, over `profiled`'s `Profiled`), so under the profile a resume
  probes it under, and every account the pools file configures for its agent found signed in.
- `prepare` builds the command and refuses it unless it carries no environment overlay, its
  `--model` and `--effort` are exactly the recorded reviewer's, and `kept` finds every guarantee
  on the command and settings actually built. A claim is never enough on its own: the check
  reads the arguments and the settings, not the adapter's word.
- `unanswered` reads the answer file again, immediately before the run is claimed and the call
  started, so an answer a person published while the pre-flight ran is seen and nothing is spent.

The prompt is built from the run's own record alone (`build_prompt`): no file of the checkout is
read for it, the plan file included. The Runner composes the call's environment for the checkout
it starts in, the one the command was run in, as it does a probe's
(`host::starts_in_the_started_checkout`).

Only Claude Code claims the guarantees (`guarantees_of`). Copilot and Codex report no dollar
cost, so a run whose primary reviewer is either is refused. What the decide call adds to an
ordinary reviewer's command is its own flags (`call_args`) and deny rules (`call_settings`), on a
profile built for the call; `review::profile_for`, `claude::build_args` and
`claude::permission_settings` are unchanged, so every ordinary reviewer and worker is given what
it was given before (`a_decide_call_adds_only_its_own_flags_to_an_ordinary_reviewers_command`).

What this module never does, and the reasons:

- It never answers. It calls no `interaction::write_answer`, no `AnswerSource` and no
  coordinator, and writes nothing under a run's public directory or an answers directory, and
  nothing in its private directory but its claim; the run lock it takes is `RunLock`'s own, whose
  files the run's engine created already
  (`a_decide_leaves_every_file_of_the_run_as_it_was_and_writes_no_answer`). A proposal
  is advice for a person; applying it stays `upstroke answer`'s job, which checks the reply
  against the run as it is when the person applies it.
- No engine code calls it, and only the CLI names it
  (`only_the_cli_names_the_decider_and_no_engine_file_reaches_it`). `decide` is denied by path in
  `clippy.toml`, so a module that states no allowance cannot reach it.
- It asks no agent about a question that binds one (`OpenAsked::binds_an_agent`). Which agent
  runs a task is chosen only through an option the run froze, and an agent choosing agents is
  not a decision this command delegates
  (`a_binding_question_is_refused_and_no_request_is_made_for_it`).
- It asks nothing of a run it does not hold. The lock is the engine's `RunLock`, not a second
  copy of it, so a resume and a decide of one run exclude each other exactly as two resumes do
  (`a_resume_cannot_take_the_run_while_decide_holds_it`,
  `a_driven_run_is_refused_before_anything_is_written_probed_or_asked`).

Effects and their containment are stated in `effects/allowlist.toml`'s funnel row.

## `pub const INPUT_BOUND: usize = 64 * 1024;`

Each prompt input (the question, the task's text, its last failure, the earlier answers) is cut
to this many bytes on a character boundary, and the prompt says where and of how many bytes
(`an_input_is_cut_on_a_character_boundary`). A task's text or a failure can be large; one bound
per input keeps a long input from crowding out the others.

## `pub const DEFAULT_MAX_COST_USD: f64 = 5.0;`

The default of `--max-cost-usd`: the dollar allowance of the run. A run is decided by one command
(`Claim::take`), from whichever copy of the checkout, with one call, so the allowance is that
call's own `--max-budget-usd` (`call_args`),
whole, checked on the command as built (`kept`), and the adapter itself stops the call at the
allowance rather than after it. A clean, successful, priced call is recorded at its price, above
its ceiling included (`the_one_call_is_capped_at_the_allowance_and_counted_at_its_price`). No
spend is summed and nothing is subtracted, so no floating-point remainder can start a call an
exhausted allowance should not (`a_decimal_allowance_buys_one_call_and_no_remainder_starts_another`).
The report's `cost` is what the call settled at, or `unknown` when it did not settle.

## `pub struct DecideOptions {`

`question_id` is required: the CLI takes it as a positional argument, as typed
(`decide_takes_exactly_one_question_id_as_typed` in `main.rs`). `run_id` is the run's exact id,
and nothing is built from it until `plain_id` admits it.
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
neither could be recorded at its price or stopped at a ceiling. A claim admits the
adapter only as far as `kept` then finds it true of the command
(`each_built_in_adapter_is_admitted_only_when_it_keeps_every_guarantee`).

## `fn call_args(ceiling_usd: f64) -> Vec<String> {`

The flags a decide call appends to the read-only profile's command, as `extra_args` of a profile
built for that call alone: `--max-budget-usd` with the run's allowance, written as Rust's
shortest round-tripping decimal so `kept` reads back exactly the ceiling it was given, and
`--strict-mcp-config`, which with no `--mcp-config` starts no MCP server. The pre-flight probes the
CLI as a run's does, for the flags every worker is built with, and not for these two; a Claude
Code too old to know one is expected to reject it as an unknown option rather than run without
it. No test here starts a real CLI: the probes are answered by inert recording Runners.

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

The admission of the run's recorded reviewer environment, under the run's lock (`decide`), in
this order, each refusal saying that nothing was written and no agent was asked. A run something
else holds never reaches it: `decide` could not take the lock.

1. A run recorded on the container runner (`RunStarted4.runner.kind`). Decide starts its
   reviewer on this host only, which is not where that run's reviewers ran
   (`a_container_run_is_refused_before_anything_is_written_or_asked`).
2. `preflight::refuse_unusable_profiles` over the pools file's pools and the recorded runner
   policy, the credential check a resume of the run makes, in its words
   (`a_run_the_credential_preflight_refuses_is_refused_before_anything_is_written_or_asked`).
3. An adapter that does not claim every `Guarantee`, naming the ones it lacks.
4. A `--max-cost-usd` that is zero, negative, infinite or not a number, which could be no call's
   ceiling (`a_cap_that_is_not_a_positive_number_of_dollars_is_refused_before_anything`).
5. A private directory `closed_private_dir` does not admit: not a real directory, or on Unix
   not the user's own or open to group or other. The claim relies on that directory's mode, so
   it is checked here, before the pre-flight probes, and again before the claim and each write
   (`a_private_directory_open_to_another_user_is_refused_before_anything`).
6. A private directory `private_denials` cannot name
   (`a_private_directory_that_cannot_be_spelled_in_a_rule_is_refused_before_anything`).

## `pub struct Decided {`

What a command that saved its question's proposal returns: the rendered report, the `question`
whose proposal it wrote, and whether that proposal is `refused`, its reply or its call refused
and its reason saved in it. Every other end of the command is an error and saves no proposal, so
the exit says a reason is saved only for a proposal that was written
(`the_exit_report_names_only_the_record_this_command_wrote`). `outcome` makes the command an
error when the proposal was refused; the CLI prints the report first, so the question's lines are
shown before the command fails. The error passes through `util::terminal::one_line`, as the
report's lines do. The lists of saved, unsaved and left questions a multi-question command
needed are gone with the loop that filled them.

The report is built one whole line at a time with `util::terminal::TerminalLines`, the builder
`status` uses. What the agent wrote (guidance, confidence, reasons, citations, and a refusal's
reason, which can quote the adapter's reading of the output), the run's question and option text,
and the file names and errors the report names are data inside a line: a line break in them is a
space and
any other control character is shown escaped, so none can start a report line of its own, clear
the screen or move the cursor (`agent_text_is_rendered_inside_its_own_line_with_its_controls_made_visible`,
`a_decide_report_shows_agent_text_as_data_and_the_proposal_keeps_it_as_written`). The saved
proposal keeps the text exactly as the agent wrote it. The report names the proposal saved
(`saved:`) and the claim, and points a person at `upstroke answer`.

## `fn refused(message: String) -> UpstrokeError {`

Every refusal this module builds is passed through `util::terminal::one_line`, and `decide` passes
the refusals the pre-flights it runs build themselves (a signed-out account's, a pools file's
profile) through the same function (`one_lined`). Every other error `propose` returns, a
configuration error among them (a pools file with a TOML syntax error, whose diagnostic spans
lines), becomes a refusal of its own words there, one line too
(`malformed_fixture_pools_refusal_is_one_line`). A refusal quotes a run's or a question's id, a
path, an error, or what the reviewer's CLI printed when a probe failed; none of them can start a
line of its own or reach the terminal as a control, as the report's lines cannot
(`every_refusal_is_one_line_whatever_the_reviewer_answered`).

## `pub fn decide(`

Checks the run id, takes the run's lock, reads the run, requires it parked, chooses the question
and hands it to `propose`. The run id is admitted by `plain_id` before any path is built from
it, so an absolute path (an archived run's directory elsewhere, say) or a `..` never replaces the
repository's runs prefix and no log outside it is read
(`a_run_id_that_is_not_one_is_refused_before_any_path_is_built`). An unknown run, a run whose
lock cannot be taken, a schema 1-3 run, a run that cannot be read and a run that is not parked
are refused with a message saying which, before any agent is asked.

The lock is `rundir::RunLock::acquire` on the run's public directory, the call `resume` makes;
nothing of it is reimplemented here. It is taken before the log is read, so what admission reads
is what a coordinator cannot change under it, and it is held, as a local, until `decide` returns:
after the proposal is written or the command has failed, panics included. A resume that starts
while the reviewer is asked is refused as a second engine would be
(`a_resume_cannot_take_the_run_while_decide_holds_it`), and a run a coordinator holds is refused
before anything is read, written, probed or asked
(`a_driven_run_is_refused_before_anything_is_written_probed_or_asked`). Because this process
holds the run, `observe` reads it as running; nothing here reads that flag.

## `fn parked(observed: &Observed, run: &str) -> Result<(), UpstrokeError> {`

The run must be parked as the fold of its log reports it: `Observed::outcome`, the
`run_finished` outcome the fold replayed, is `Parked`. An open question and a free lock are not
that: a run interrupted with an open question and another runnable task has no recorded end,
`status` calls it interrupted, and a resume would carry it on, so it is refused with nothing
spent (`an_interrupted_run_with_an_open_question_is_refused_before_anything`). A run that ended
complete, halted or over its budget is refused as well.

## `fn propose(`

One invocation through `runner::decide_request`, with `InvocationId::Decide { ordinal: 1 }`
(`CALL_ORDINAL`), the adapter's built command with its stdin payload and settings file, and no
environment overlay (`the_request_is_a_decide_request_for_the_runs_reviewer_with_no_overlay`). A
question that binds an agent is refused first; its id is never checked as a file name, since none
is named after it. `admit` runs after the pools file is read. `unanswered` then refuses a question a person
has answered, with nothing probed or claimed
(`a_question_a_person_has_answered_is_refused_and_never_asked`). The recorded reviewer's
pre-flight runs (`preflight::certify_agent`) and refuses a CLI that cannot be probed or an account
that is signed out
(`the_recorded_reviewer_passes_the_runs_own_preflight_before_anything_is_written_or_asked`). The
call is built and checked by `prepare`, with the allowance as its ceiling, before anything is
written. `unanswered` reads the answer file again, immediately before `Claim::take` claims the
run and `ask` writes and starts the call, so an answer published during admission is seen and no
paid call is made (`an_answer_published_after_admission_is_seen_before_the_call`).

Until the claim, every refusal is the command's error, with nothing written and no agent asked.
After it, a call that returns an outcome that does not settle is saved as refused, and a file that
cannot be written, or that `Claim::admits` no longer admits, fails the command naming the
question, with no proposal saved (`a_taken_name_is_never_replaced_and_fails_the_command`,
`a_claim_changed_while_the_reviewer_runs_is_not_written_into`). The claim stays in every case.

Once the pools file is read, the Runner the command was handed is wrapped
(`profiled::profiled_borrowing`) in the `Profiled` wrapper a resume installs over the runner its
pre-flight probes through, and the pre-flight's probes and the call both go through it. So each
carries the profile of the first pool of the reviewer's agent (`capacity::pool_for`), as a
resume's probes and a review pass of the run would: a CLI that fails its probe under that profile
is refused here as a resume's pre-flight refuses it, though it passes under the default account,
and no paid request is made (`same_probe_refusal_for_configured_profile`). A further account's
sign-in discovery keeps the profile pre-flight gave it. The call's profile used to be set by hand
on its request while the probes went through the bare Runner; with one wrapper for both, that
second lookup is gone. The timeout is the run's recorded review pass timeout, or the review
default. Split from `decide` so a test can hand it a binding question.

The per-question loop is gone, and with it what it needed: the running spend and the
remaining-allowance subtraction, the set of questions already called, and the lists of
questions left, unsaved and saved.

## `fn prepare(`

Builds one call: the read-only profile with `call_args`, the settings `call_settings` gives, and
the adapter's command with its stdin payload and the settings file named. It writes and starts
nothing, and refuses the call unless the built command carries no environment overlay (naming the
overlay's keys and never their values), carries exactly the recorded reviewer's `--model` and the
run's recorded review effort as `--effort` (another model, another effort or none is refused, and
the refusal names the recorded pair), and `kept` holds for every `Guarantee`. The settings
path is the claim's `settings.json` (`SETTINGS_FILE`), beside the proposal. The call is prepared
before the run is claimed, so its refusal leaves nothing written
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
- A per-call dollar ceiling: `--max-budget-usd` with a finite value above zero and at most the
  run's `--max-cost-usd` allowance.
- No web access: `--permission-mode dontAsk`, which denies every tool the settings do not allow;
  `--setting-sources ""`, so no user, project or local settings widen them; `--strict-mcp-config`;
  settings that allow reading only; and `WebFetch` and `WebSearch` denied by name.
- The run's private data unreadable: `--setting-sources ""`, settings that allow reading only, the
  profile's `Read(.upstroke/**)` and `Read(**/.upstroke/**)`, and every rule `private_denials`
  wrote.

## `enum Returned {`

What the call returned. `Settled` is a clean, successful, priced call, and only it is settled,
at its price; `Unsettled` is every other outcome, with any cost it reported, which the proposal
records as reported and unsettled.

## `fn settle(adapter: &dyn AgentAdapter, output: &ProcessOutput) -> Returned {`

A call settles when its process exited 0, within its time limit and its output limit, the adapter
read the outcome as `Completed`, and the outcome reports a finite cost of $0 or more. Anything
else does not, and the reason names the first thing missing: a timeout, an output limit, a
non-zero exit or a signal, an outcome the adapter read as an agent error, a timeout or a rate
limit, a cost that is no price, or no cost at all; a Runner error is read in `ask`
(`only_a_clean_successful_priced_call_settles_and_any_other_is_saved_refused`). A settled call
whose reply `check_reply` refuses is still settled: what it cost is known. An unsettled one is
saved as refused, its cost `unknown` in the report.

## `fn ask(`

Writes the call's settings file new, starts the command's one call through the Runner as
`CALL_ORDINAL`, saves its raw output
new, and reads what it returned with `settle`. It takes the `Call` by value: the command and
workspace `prepare` checked move into the request, uncopied. A Runner error (the process could not be started,
was cancelled, or ended in a way the Runner reports as an error) is a returned outcome too: its
words are saved in place of the raw output, and it does not settle. Both files go through
`Claim::write`, so each is written immediately after the check that admits it: the raw output's
check is made once the call has returned, not before the call, so no reviewer call sits between a
check and its write. A launcher killed mid-call returns nothing, and leaves the claim and the
settings file in place; the run's lock is given back as the stack unwinds, or by the OS when the
process dies
(`the_run_is_claimed_in_its_private_directory_before_its_first_call_and_the_claim_outlives_an_interruption`).

## `fn plain_id(id: &str) -> bool {`

A run id is admitted only when it is 1 to `MAX_ID_BYTES` ASCII letters, digits, `-` or `_` and
no Windows device name (`CON`, `NUL`, `COM1`, ...): no separator, no `.`, no `..`. It is checked
first of all, before any path is built from it: the run's public directory and its lock are named
after it (`a_run_id_is_a_plain_file_name_component_or_no_path_is_built_from_it`). A question id is
not checked: no file is named after it.

## `fn real_dir(path: &Path) -> Result<std::fs::Metadata, String> {`

A directory decide relies on is read without following a link and must be a directory: a symlink
or a file at the name is refused, and on Windows so is a junction or any other reparse point,
because `reparse_point` reads the raw `FILE_ATTRIBUTE_REPARSE_POINT`, set for every reparse point
whatever its tag.

## `fn this_users(metadata: &std::fs::Metadata, closed: bool) -> Result<(), String> {`

On Unix the directory must belong to the user decide runs as, its effective uid (`geteuid`, which
`effects/wrappers.toml` lists as no effect), which owns every file decide creates; when `closed`,
its mode must grant group and other nothing, no bit of `077`. Elsewhere std reads no owner and no
mode, and this checks nothing: on Windows only `real_dir`'s check is made, and a directory keeps
whatever access list the system gave it.

## `fn closed_private_dir(private: &Path) -> Result<(), String> {`

The claim relies on its parent's mode. `Claim::take` creates the claim directory with a plain
`create_dir`, at whatever mode the umask gives it, and no mode is changed afterwards, so what keeps
the claim and its files from every other user is the run's private directory alone. This is the
check of that directory: `real_dir`, then `this_users` closed. `admit` makes it before the
pre-flight probes, `Claim::take` immediately before the `create_dir`, and `Claim::admits` before
each file is written (`a_private_directory_open_to_another_user_is_refused_before_anything`). The
engine creates the private directory at the umask's mode (`rundir::create_private_dir`), so under
a umask such as `022` decide refuses the run until a person closes the directory.

## `struct Claim {`

The run's claim: `decide/` (`CLAIM_DIR`) in the run's private directory, the one `run_started`
recorded, which every copy of the checkout names. The claim holds three fixed names,
`settings.json`, `raw.out` and `proposal.json`; with one proposal per run no question id becomes a
file name, so ids that differ only by case cannot collide on a filesystem that folds case
(`no_question_id_becomes_a_file_name`,
`the_exact_question_is_asked_when_ids_differ_by_case`).

`take` is the run's one claim, made immediately before its call: `closed_private_dir`, then
`create_dir` of the claim, which is exclusive. Whatever stands at the name already is refused,
never adopted, and refuses the command with nothing written and no agent asked: an earlier
decide's claim, complete, partial or interrupted, an empty one, a file or a link
(`a_run_whose_claim_exists_is_not_asked_and_nothing_is_written`). Of two commands started beside
each other, from one copy of the checkout or two, the one whose `create_dir` loses is refused the
same way (`the_exclusive_create_dir_is_what_makes_a_second_command_find_the_claim`). Nothing in
the claim is read: its existence is the whole answer. The private directory is then synced, so the
claim is durable before the call
(`the_private_directory_is_synced_once_the_claim_is_created_in_it`); a failure there still leaves
the claim in place, and the run claimed. The claim is never removed, after a failure or an
interruption, so a run is asked once, about one question. No directory is created in the checkout,
so a directory an older decide left there stops nothing
(`a_directory_left_open_in_the_checkout_stops_no_decide`).

`admits` is the check that admits each write: `closed_private_dir` again, and the claim still a
real directory of the user decide runs as (its own mode is not checked). `write` makes it
immediately before it creates the file, so a claim replaced by a link, or a private directory
opened to its group, while the reviewer ran is seen and nothing is written into it
(`a_claim_changed_while_the_reviewer_runs_is_not_written_into`).

## `fn save(claim: &Claim, record: &SaveRecord<'_>) -> Result<(), UpstrokeError> {`

The run's one proposal, `proposal.json`, created through `Claim::write`. It records the run, the
question, the decider, `status` with its `record` or `reason`, the `cost_usd` reported, whether
the call `settled`, its `ceiling_usd`, and `raw_output`, the raw output's fixed name `raw.out`,
beside it in the claim, which names that file wherever the private directory is.

## `fn unanswered(answers: &Path, asked: &OpenAsked) -> Result<(), UpstrokeError> {`

Refuses the question when a person has answered it already: an answer `upstroke answer`
published in the run's answers directory that the run's log has not settled yet, read with
`interaction::read_answer`, which writes nothing. It is read twice, both before the run is
claimed, so the check never follows a write: at admission, so an answered question is not even
probed for, and again immediately before the claim and the call, so an answer published while the
pre-flight ran is not paid for. An answer file that cannot be read refuses the command with
nothing written. The refusal names the answer, which the run applies when it next reads it.

## `fn build_prompt(observed: &Observed, asked: &OpenAsked) -> String {`

The prompt holds only what the run recorded: the question, the task's text from the run's
registry, its last settled attempt and its earlier answers, all read through `observe` from the
run's log and its frozen plan, which `observe` adopts only when it digests (SHA-256, every byte)
to the `normalized_plan_digest` the run recorded. No plan file of the checkout is read. The plan
file used to be forwarded when its bytes hashed to the run's `plan_hash`, but that hash,
`ir::content_hash`, drops every `\r`, so a plan file given a paragraph break of two lone carriage
returns hashed as the recorded plan and its edited bytes reached the reviewer; the read and its
check are gone rather than guarded (`plan_with_an_added_paragraph_break_is_withheld`).

## `fn chosen_question(`

The open question whose id is the one given, compared byte for byte: no case is folded and no
prefix matched, so of two open questions `q-A` and `q-a` the one named is the one asked, and the
run's one claim is spent on it (`the_exact_question_is_asked_when_ids_differ_by_case`). A
question the run's log has settled is refused as no longer open, and any other id as one the run
does not have, each with nothing done
(`runs_and_questions_that_cannot_be_decided_are_refused_before_any_request`).

## `enum RaisedBy {`

Who raised a question, read from the event that opened it (`RaisedBy::of`), and the prompt's
`raised by:` line. Only an attempt that settled parked on a `NeedsHuman` failure of worker origin
is the worker's: the worker itself asked for a person. Every other question the coordinator
raised, and the prompt says so and that the worker did not ask it: a parked attempt whose failure
came from a reviewer's verdict, or whose ladder or deferrals ran out, or whose review input was
too large or opaque; `question_raised`, which the coordinator writes to approve a spend onto the
frontier rung; a spawned or repaired task's admission question; and a parked integration
verification. The failure section of the prompt still names a reviewer where one asked
(`a_question_is_labelled_by_whether_the_worker_or_the_coordinator_raised_it`). An open question
always has an opening event in the log the fold read; if none were found, the line says the
raiser is unknown rather than guessing.

## `fn check_reply(question: &Question, text: &str) -> Result<Value, String> {`

Exactly one fenced `json` block, with no line outside it holding a fence (`fenced_json`); all six
fields, their types, and a choice and guidance the run could apply as a person's answer. A choice
outside the options, a choice `interaction::answer_for_option` maps to a decline (or whose text
`interaction::interpret` reads as one), and `null` where the question has options are refused
(`a_bad_reply_is_refused_with_its_reason`,
`each_bad_reply_is_refused_saved_and_fails_the_command`). So is guidance that
`interaction::interpret` reads as declining, the reading `upstroke answer --text` gives it: `skip`,
`decline`, `fail` or `abandon` alone in any case, or the number of an option that gives up.
Guidance that merely mentions such a word, or names an option that does not give up, is advice
(`guidance_read_as_declining_the_task_is_refused`). The prompt says the same of `guidance`.

## `fn fenced_json(text: &str) -> Result<&str, String> {`

The reply's one block opens at the first line that is ```` ``` ```` and the info string `json`
(a space between them allowed), and closes at the next line that is ```` ``` ```` alone, each with
nothing beside it but whitespace and a carriage return. Nothing inside the block is read as a
fence, so the JSON may mention ```` ``` ```` or `~~~` in a string: a JSON string holds no raw line
break, so no line of valid JSON is ```` ``` ```` alone, and the closing line is never the JSON's
own (`a_fence_mentioned_inside_the_json_block_is_its_text_and_only_a_line_alone_closes_it`).

Every line outside the block, before it opens or after it closes, is refused when it holds three
backticks or three tildes anywhere in it, a fence of another block or a run within a line of
prose alike: Markdown could read either as something fenced, and the prompt asks for nothing
fenced outside the one block and says no line outside it may hold either run. The block's own
closing line is the block's, never outside text, so a reply whose closing ```` ``` ```` is its
last line is accepted, while a second ```` ``` ```` line after it is refused. The refusal names
the line and whether it came before the block opened or after it closed at which line. A second
`json` block, a block of any other info string or none, a block fenced with tildes, a block left
open after the proposal and a line of inline code are each refused that way; a reply with no
opening line, and a block whose closing line has text beside it or more backticks than three, and
so is never closed, are refused as such (`a_reply_is_one_json_block_and_no_line_outside_it_holds_a_fence`,
whose cases are replies written out in plain text).

## `fn create_durably(path: &Path, bytes: &[u8]) -> std::io::Result<()> {`

Every file this command writes, each settings file, raw output and proposal, is created new
(`File::create_new`), written, and synced with its directory; it never overwrites, and a name that
is taken is an error rather than a replacement, which fails the command with the file as it was
(`a_taken_name_is_never_replaced_and_fails_the_command`). `write_new` is
the same with the error naming the path.
