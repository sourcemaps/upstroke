# Decide

[`src/decide.rs`](../../src/decide.rs) is `upstroke decide <run-id> [<question-id>]`: it asks a
run's recorded primary reviewer to propose an answer to each of the run's open questions, saves
each reply under `.upstroke/proposals/<run-id>/`, and answers nothing. A person applies a
proposal with `upstroke answer`.

A run is decided once. The command claims it by creating `.upstroke/proposals/<run-id>/`
exclusively, immediately before its first call, and never reads that directory or removes it: a
run whose directory exists, from a complete, partial or older decide or anything else, is
refused with nothing written and no agent asked (`claim`). So the accounting is this command's
own, in memory: each question gets at most one call, each call is capped at what is left of
`--max-cost-usd`, and only a clean, successful, priced call is counted, at its price
(`settle`). Any other outcome a call returns ends the command, and a command that stops with
questions left names them and exits non-zero (`Decided::outcome`).

## Module

The run is read through `engine::topology::observe`, the reading `answer` and `status` use, and
only a schema-4 run is accepted. The decider is `RunStarted4.reviews.primary` at
`RunStarted4.effort_policy.review`; a run with no primary binding is refused and nothing else is
chosen. Its profile is `review::profile_for`'s, which is read-only. Because that profile cannot
read `.upstroke/**`, and a decide call cannot read the run's private directory either, every
input the decider needs is handed to it in the prompt.

Admission comes before the run is claimed, anything is written or any agent is asked, in these
steps:

- Once per command, `admit` refuses a run an engine is driving (`rundir::is_running`), a run
  recorded on the container runner, a run whose pools the credential pre-flight a resume runs
  (`preflight::refuse_unusable_profiles`) refuses, a reviewer whose adapter does not claim every
  `Guarantee`, a `--max-cost-usd` that is not a positive number, and a private directory no
  permission rule can name exactly.
- Once per command, `answered` reads each question's answer file (`interaction::read_answer`), so
  a question a person has answered already is skipped and never asked.
- Once per command, when a question is left to ask, the recorded reviewer passes the pre-flight a
  run makes of each recorded agent (`preflight::certify_agent`, which `RunPreflight::certify`
  runs too): its CLI probed through the Runner, and every account the pools file configures for
  its agent found signed in.
- Once per call, `prepare` builds the command and refuses it unless it carries no environment
  overlay, its `--model` and `--effort` are exactly the recorded reviewer's, and `kept` finds
  every guarantee on the command and settings actually built. A claim is never enough on its
  own: the check reads the arguments and the settings, not the adapter's word.

The plan file is forwarded in the prompt only when its bytes hash to the run's recorded
`plan_hash` (`recorded_plan`). The Runner composes the call's environment for the checkout it
starts in, the one the command was run in, as it does a probe's
(`host::starts_in_the_started_checkout`).

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

## `pub const DEFAULT_MAX_COST_USD: f64 = 5.0;`

The default of `--max-cost-usd`: the dollar allowance of the run. A run is decided by one command
(`claim`), so the command's allowance is the run's aggregate cap, and it is kept in memory: no
file is read back to learn what was spent. Each call is given what is left as its own
`--max-budget-usd` (`call_args`), checked on the command as built (`kept`), so the adapter itself
stops a call at the allowance rather than after it
(`each_calls_ceiling_is_what_is_left_of_the_cap`). A clean, successful, priced call is counted at
its price, above its ceiling included; once nothing is left the next question is not started, and
the report names it and every question after it
(`the_allowance_stops_the_next_question_and_the_command_names_it_and_fails`). Any other outcome
ends the command before another call, since what it spent is unknown (`settle`). The report's
`total cost` is what the settled calls cost; when a call did not settle it is named, and the total
is a floor.

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

The once-per-command admission, in this order, each refusal saying that nothing was written and
no agent was asked:

0. A run an engine is driving: `rundir::is_running` on its public directory, which also answers
   yes when whether one is cannot be told. A driven run settles its own questions, so it is not
   read for questions to ask while it may be settling them, and nothing is probed
   (`a_driven_run_is_refused_before_anything_is_written_probed_or_asked`).
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

The rendered report and four lists of question ids, each naming only what the command did:
`saved`, the questions whose proposal it wrote, accepted or refused, in the order written;
`refused`, the questions whose reply or call was refused, each of them in `saved` with its reason
in its proposal; `unsaved`, the questions refused before any agent was asked because no file can
be named after their id (`plain_id`), for which nothing is written; and `left`, the questions the
command stopped before proposing an answer to: the question whose call was in flight when a write
failed, and every question after the one it stopped at whose proposal is not saved already. So
the exit says a reason is saved only for a proposal that was written, and says nothing is saved
for a question refused before it was asked
(`the_exit_report_names_only_the_records_this_command_wrote`). `outcome` makes the command an
error when any was refused, unsaved or left, naming them; the CLI prints the report first, so
every question's line is shown before the command fails. The error's lists of ids are passed
through `util::terminal::one_line`, as the report's lines are.

The report is built one whole line at a time with `util::terminal::TerminalLines`, the builder
`status` uses. What the agent wrote (guidance, confidence, reasons, citations, and a refusal's
reason, which can quote the adapter's reading of the output), the run's question and option text,
and the file names and errors the report names are data inside a line: a line break in them is a
space and
any other control character is shown escaped, so none can start a report line of its own, clear
the screen or move the cursor (`agent_text_is_rendered_inside_its_own_line_with_its_controls_made_visible`,
`a_decide_report_shows_agent_text_as_data_and_the_proposal_keeps_it_as_written`). The saved
proposal keeps the text exactly as the agent wrote it. The report ends naming the proposals
saved (`proposals saved:`, or `none`), and it points a person at `upstroke answer` only when one
was.

## `fn refused(message: String) -> UpstrokeError {`

Every refusal this module builds is passed through `util::terminal::one_line`, and `decide` passes
the refusals the pre-flights it runs build themselves (a signed-out account's, a pools file's
profile) through the same function (`one_lined`). A refusal quotes a run's or a question's id, a
path, an error, or what the reviewer's CLI printed when a probe failed; none of them can start a
line of its own or reach the terminal as a control, as the report's lines cannot
(`every_refusal_is_one_line_whatever_the_reviewer_answered`).

## `pub fn proposals_dir(repo_root: &Path, run_id: &str) -> PathBuf {`

`<repo>/.upstroke/proposals/<run-id>`. The name `proposals` appears in production code only here.

## `pub fn decide(`

Reads the run, chooses its questions and hands them to `propose`. The run is found by its exact
id; an unknown run, a schema 1-3 run and a run that cannot be read are refused with a message
saying which, before any agent is asked.

## `fn propose(`

At most one invocation per question through `runner::decide_request`, with
`InvocationId::Decide { ordinal }` numbered by the question's place in this command's order, the
adapter's built command with its stdin payload and settings file, and no environment overlay
(`the_request_is_a_decide_request_for_the_runs_reviewer_with_no_overlay`). `admit` runs first,
after the pools file is read and before anything else, and a run id that is not a plain component
is refused next. `answered` then reads which questions a person has answered; each is skipped with
its answer named, and with none left to ask nothing is probed or claimed
(`a_question_a_person_has_answered_is_skipped_and_never_asked`). When one is left, the recorded
reviewer's pre-flight runs (`preflight::certify_agent`) and refuses a CLI that cannot be probed or
an account that is signed out
(`the_recorded_reviewer_passes_the_runs_own_preflight_before_anything_is_written_or_asked`).
Each question's call is then built and checked by `prepare`, with what is left of
the allowance as its ceiling, before anything is written for it; before the first call `claim`
claims the run, and `ask` writes and starts the call. A question whose id is not a plain component
(`plain_id`) is refused before anything is written for it, and counted as unsaved rather than
refused; one listed twice is asked once (`a_question_listed_twice_is_asked_once`), and once its
proposal is saved it is never named as left (`left_from`). A run with no question to ask is never
claimed (`a_binding_question_is_skipped_and_no_request_is_made_for_it`).

Until the claim, every refusal is the command's error, with nothing written and no agent asked.
After it the command never returns an error: it stops, and the report says why and names the
questions left. It stops when the allowance is spent, when a call returns an outcome that does
not settle (saved as refused first), and when a file cannot be written or a later call cannot be
built, the question in hand counted as left
(`a_taken_name_is_never_replaced_and_ends_the_command_naming_the_questions_left`). The claim stays
in every case. The credential profile is the first pool of the reviewer's agent
(`capacity::pool_for`), as a review pass of the run would use; the timeout is the run's recorded
review pass timeout, or the review default. Split from `decide` so a test can hand it a binding
question or a list naming one question twice.

## `fn prepare(`

Builds one call: the read-only profile with `call_args`, the settings `call_settings` gives, and
the adapter's command with its stdin payload and the settings file named. It writes and starts
nothing, and refuses the call unless the built command carries no environment overlay (naming the
overlay's keys and never their values), carries exactly the recorded reviewer's `--model` and the
run's recorded review effort as `--effort` (another model, another effort or none is refused, and
the refusal names the recorded pair), and `kept` holds for every `Guarantee`. The settings
path is `<id>.settings.json` beside the proposal (`Names`). The first question's call is
prepared before the run is claimed, so its refusal leaves nothing written
(`a_mocked_adapter_whose_command_breaks_a_promise_is_refused_before_anything`); the command's
shape does not vary by question, so a later refusal would mean the adapter changed between calls,
and it still refuses that call before it is written or started and stops the command.

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

## `enum Returned {`

What one call returned. `Settled` is a clean, successful, priced call, and only it is counted,
at its price; `Unsettled` is every other outcome, with any cost it reported, which the proposal
records and the command never counts.

## `fn settle(adapter: &dyn AgentAdapter, output: &ProcessOutput) -> Returned {`

A call settles when its process exited 0, within its time limit and its output limit, the adapter
read the outcome as `Completed`, and the outcome reports a finite cost of $0 or more. Anything
else does not, and the reason names the first thing missing: a timeout, an output limit, a
non-zero exit or a signal, an outcome the adapter read as an agent error, a timeout or a rate
limit, a cost that is no price, or no cost at all; a Runner error is read in `ask`
(`only_a_clean_successful_priced_call_settles_and_any_other_ends_the_command`). A settled call
whose reply `check_reply` refuses is still settled: what it cost is known, and the next question
is asked with what is left. An unsettled one is saved as refused and ends the command, because
what it spent is unknown and the next call's ceiling could not be what is left.

## `fn ask(`

Writes the call's settings file new, starts the call through the Runner, saves its raw output
new, and reads what it returned with `settle`. It takes the `Call` by value: the command and
workspace `prepare` checked move into the request, uncopied. A Runner error (the process could not be started,
was cancelled, or ended in a way the Runner reports as an error) is a returned outcome too: its
words are saved in place of the raw output, and it does not settle. A launcher killed mid-call
returns nothing, and leaves the claim and the settings file in place
(`the_run_is_claimed_owner_only_before_its_first_call_and_the_claim_outlives_an_interruption`).

## `fn plain_id(id: &str) -> bool {`

A question id is written into a file name only when it is 1 to `MAX_ID_BYTES` ASCII letters,
digits, `-` or `_` and no Windows device name (`CON`, `NUL`, `COM1`, ...): no separator, no `.`,
no `..`. Without a `.` in an id, `<id>.json`, `<id>.out` and `<id>.settings.json` name one
question's files exactly, and no question's file is spelled like another's. The run id is held to
the same rule before the run's directory is named after it. Ids the engine mints are `q-<ulid>`
(`an_untrusted_proposals_directory_or_a_question_id_that_is_not_plain_is_refused`).

## `enum Privacy {`

What a directory of the chain keeps from every user but its owner. `Closed` is `.upstroke`: the
run created it with the umask's mode, other commands read the run's public half under it, and
decide neither creates nor changes it, so it requires only that no other user can write it, and
so none can add, rename or replace `proposals`. `OwnerOnly` is `.upstroke/proposals` and the run's
own directory, which decide creates with mode `0700` and which no other user may read, write or
enter, so nobody else can read a proposal or put a file where one is to be created.

## `fn trusted_dir(path: &Path, repo_root: &Path, privacy: Privacy) -> Result<(), String> {`

Each directory of the chain is read without following a link and must be a directory: a symlink
or a file at the name is refused, and so is a Windows reparse point (`owned_and_private`). On
Unix it must also belong to the repository root's owner and grant no other user what its
`Privacy` withholds (`the_chain_is_the_owners_and_closed_to_every_other_user`), so the path from
the repository to every file decide creates is the owner's alone and nothing needs pinning while
the command runs. std reads no owner or access list on Windows, so there the chain has the ones
the repository's own directory passes down.

## `fn ensure_dir(repo_root: &Path, dir: &Path, privacy: Privacy) -> Result<(), UpstrokeError> {`

`.upstroke` and `.upstroke/proposals`, one level at a time with `create_dir` (never
`create_dir_all`, which would create through a link): created owner-only where missing, then
checked and their parent synced whoever created them, the run, another caller or this one, so
each directory's entry is durable before anything is created under it
(`each_parent_is_synced_whoever_created_it`). A failure here leaves the run unclaimed.

## `fn claim(repo_root: &Path, run_id: &str, dir: &Path) -> Result<(), UpstrokeError> {`

The run's one claim, made immediately before its first call: `ensure_dir` for the two
directories above, then `create_dir` of `.upstroke/proposals/<run-id>`, which is exclusive.
Whatever stands at the name already refuses the command with nothing written and no agent asked:
an earlier decide's directory, complete, partial or of an older format, an empty one, a file or a
link (`a_run_whose_directory_exists_is_not_asked_and_nothing_is_written`). Nothing in the
directory is read: its existence is the whole answer, so a person settles what is there and
nothing is inferred from it. The new directory is made owner-only, checked, and its parent synced;
a failure there still leaves it in place, and the run claimed. The directory is never removed,
after a failure, a stop or an interruption, so a run is asked once.

## `fn reference(run_id: &str, name: &str) -> String {`

How a saved proposal names a file it generated, its raw output: the path from the repository
root, `.upstroke/proposals/<run-id>/<id>.out`, with `/` between the components on every platform.
Every component is a plain name (`plain_id` admits the run and the question), so the reference
names the same file wherever the repository is checked out and on whichever system reads it; an
absolute path would name the checkout that wrote it, in that system's spelling
(`a_saved_proposal_names_its_raw_output_wherever_the_repository_is_moved`).

## `struct Names {`

One question's three files in the run's directory: `<id>.settings.json`, the call's permission
settings; `<id>.out`, its raw output or the Runner's error; `<id>.json`, the proposal, which
records the run, the question, the decider, `status` with its `record` or `reason`, the
`cost_usd` reported, whether the call `settled`, its `ceiling_usd` and the `raw_output`
reference.

## `fn answered(`

The questions a person has answered already: an answer `upstroke answer` published in the run's
answers directory that the run's log has not settled yet, read with `interaction::read_answer`,
which writes nothing. Every question this command could ask is read before the run is claimed, so
the check never follows a write; an answer file that cannot be read refuses the command with
nothing written. A question with an answer is skipped, its answer named in the report, and it is
neither asked nor counted as left: the run applies the person's answer when it next reads it.

## `fn recorded_plan(path: &Path, plan_hash: &str) -> String {`

The plan file is forwarded only when its bytes hash (`ir::content_hash`, the hash the markdown
adapter froze it under) to the run's recorded `plan_hash`. A plan edited since the run started is
not the plan the run is executing, so the prompt carries a line saying it is not forwarded and
naming both hashes instead
(`the_plan_is_forwarded_only_when_its_bytes_are_the_plan_the_run_recorded`).

## `fn chosen_questions(`

Without an id, every open question in the order raised (`Observed::raised_questions`). With one,
the open question of that exact id or of an unambiguous prefix within the run; a question the
run's log has settled is refused in `upstroke answer`'s words.

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
`each_bad_reply_is_refused_saved_and_fails_the_command_after_the_others`). So is guidance that
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
is taken is an error rather than a replacement, which stops the command with the file as it was
(`a_taken_name_is_never_replaced_and_ends_the_command_naming_the_questions_left`). `write_new` is
the same with the error naming the path.
