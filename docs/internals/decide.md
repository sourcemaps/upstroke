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
read `.upstroke/**`, every input the decider needs is handed to it in the prompt.

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
reply with no reported cost counts as unknown and is named, so the total is a floor.

## `pub struct DecideOptions {`

`pools_path` is `None` from the CLI, which reads the pools file `upstroke capacity` reads by
default; the tests name a pools file in their scratch tree.

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
(`the_request_is_a_decide_request_for_the_runs_reviewer_with_no_overlay`). The credential profile
is the first pool of the reviewer's agent (`capacity::pool_for`), as a review pass of the run
would use; the timeout is the run's recorded review pass timeout, or the review default. A
question is not re-asked. Split from `decide` so a test can hand it a binding question
(`a_binding_question_is_skipped_and_no_request_is_made_for_it`).

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
that is taken is an error rather than a replacement.
