# `src/engine/tests/questions.rs`

Extended notes for [`src/engine/tests/questions.rs`](../../../../src/engine/tests/questions.rs).

## `let (_tree, repo) = temp_engine_repo("park");`

§21 definition-of-done (d) and invariant 6: t1 exhausts its chain
and parks; the independent t3 must still commit.

## `let source = source(`

t1 fails; every later attempt (t3's) edits and passes.

## `let path = repo`

The question is on disk where a notifier, `upstroke answer`, or a UI
can read it — that file is the contract, not the terminal output.

## `assert!(committed(&report, "t3"));`

Independent work ran while t1 waited, and the answer resumed it.

## `assert!(`

Parking rolled the tree back, so the session's account of what it
wrote no longer matches the repository (§14 pairs resume with tree
retention). The retry therefore starts fresh and carries the whole
task again, with the operator's answer as an instruction.

## `let runs = source.adapter.runs();`

Invocation order across the whole run, not just this task: t1 asks
(0), the independent t3 proceeds while t1 is parked (1), then t1
retries once the answer arrives (2). That interleaving is the point
of invariant 6, so the retry is the third invocation.

## `let (_tree, repo) = temp_engine_repo("ci");`

§12: `interaction = "never"` degrades questions to parked-task
reporting, and the outcome is distinguishable from both a clean run
and a halt.

## `let (_tree, repo) = temp_engine_repo("noloop");`

Without this the hard block spins: ask, get nothing, ask again.

## `assert_eq!(`

Multi-line questions survive, because the prompt asks for it last.

## `let reply = "The retry feedback says I can use the UPSTROKE-QUESTION: marker if I am \`

The engine hands the agent this marker in every fresh prompt, and
the empty-diff feedback names it verbatim — so an agent mentioning
it before asking is the expected shape, not a corner case. Taking
the first occurrence would hand the operator the agent's reasoning
with the question buried at the end.

## `let quoting = "I will end with the UPSTROKE-QUESTION: marker if I get stuck.";`

`detail` carries the agent's partial output on every failure path,
and that output routinely quotes the prompt. Reading the marker
before the status would turn a rate limit into a parked question —
silently defeating "RateLimited defers rather than burning an
attempt", and losing the timeout's transcript-tail feedback.

## `let mut asked = fake_outcome(`

A genuine question on a completed run still parks the task.

## `let (_tree, repo) = temp_engine_repo("haltpark");`

t1 parks on a question, t2 fails terminally under the default halt
policy. Asking about t1 afterwards spends the operator's attention
on an answer no attempt can consume, and a decline would relabel
`halted_at` with t1 — sending triage at the wrong task.

## `let source = source(`

t1 asks and parks; t2 changes nothing and parks on chain exhaustion.

## `let answers = ScriptedAnswers::new(vec![`

Declining t1 fails it, which halts the run under the default policy.
The second answer must never be consumed: t2's question cannot be
asked once nothing can act on the reply.

## `assert!(`

The distinguishing assertion. Unguarded, t2's question would be
asked, answered, and flipped back to Pending — where `next_ready`
refuses it because the run has halted, so it would surface as
`Skipped` with the operator's answer sitting unused on disk.

## `let (_tree, repo) = temp_engine_repo("answerresume");`

§21's definition-of-done (d) across processes: the run ends parked,
a person answers with `upstroke answer` while nothing is running, and
the resume picks the answer up.

## `let recorded = crate::answer::answer(`

Nothing is running; the answer is written by the CLI path.

## `let runs = source.adapter.runs();`

This adapter is fresh for the resume, so its first invocation is
t1's retry — the one the answer released. t2 runs after it.

## `let (_tree, repo) = temp_engine_repo("midrun");`

Invariant 6 at its most useful: the operator answers from elsewhere
while other work is still going, and the task is released on the
next scheduler turn rather than at the end of the run.

## `let report = run_harness(`

Nobody is reachable through the answer *channel* at all: if the
sweep did not exist, t1 could only ever end parked.

## `struct AnsweringViaFile {`

Stands in for an operator running `upstroke answer` in another terminal
while the run is still going: it writes the file and tells the engine
nobody replied, so only the sweep can find it.

## `let (_tree, repo) = temp_engine_repo("blocked");`

The chain is listed backwards on purpose: a single pass in plan
order would settle `late` before `mid` was known to be blocked, and
report it as merely skipped.

## `let (_tree, repo) = temp_engine_repo("unblock");`

Blocked is a *view*, not recorded state — which is what lets an
answer make a whole chain runnable again on resume.

## `let (_tree, repo) = temp_engine_repo("terminate");`

The drain loop's termination argument, executed: an adapter that
never succeeds, a pool that never returns, and a channel nobody
answers. Every branch of the loop fires and the run still ends.

## `struct BacklogAnswers {`

An operator working a backlog: they answer some other parked question
out of band, reply to this one at the prompt, and then walk away — so a
dropped answer never gets a second chance.

## `let (_tree, repo) = temp_engine_repo("backlog");`

Both channels can produce an answer on one scheduler turn. The sweep
must not swallow the reply the operator typed: it closed a different
question, and discarding this one throws away words a person sat and
wrote — words nothing will ask for again.

## `let source = source(`

Both tasks fail into a question, then both succeed once released.

## `let (_tree, repo) = temp_engine_repo("nullanswer");`

`sweep_answers` reports whether anything *changed*, and the drain
loop trusts that to mean it made progress. A file the sweep reads
but declines to apply — `unanswered`, which `upstroke answer` refuses
to write but a hand-edit produces — must not read as progress: that
branch terminates only because it closes the question it fires for.
A regression here hangs this test rather than failing it.

## `const QUESTION_PAYLOAD_KILL_CHILD: &str = "engine::tests::question_payload_kill_child";`

The second: the payload's write dies at one of its phases.

## `fn a_kill_at_the_question_payload_write_is_recovered_by_the_resume(`

Rows 85 and 86 of Gate 5's audit, `RunDir.WriteQuestionPayload` before and after, where the
payload's only production writer runs: the legacy engine (#292's round-1 fix-check lens, finding 1,
found the topology witness this replaced resuming a planted schema-4 run, which never runs that
recovery). The first child leaves the settlement durable with no payload; the second dies at the
phase, leaving the payload exactly where the authority's rows put R21 (absent before the write,
present after it). The legacy resume then writes or adopts it: after the after-phase kill, its
rewrite is byte-identical to what the killed write left. It appends after the durable prefix, and
its state replays equal on two loads. With the rewrite made to clear each record's options (the
lens's mutation), both witnesses fail on the payload's equality with the recorded question. The
second child is given the witness's tree as its temporary directory, as the first is.

## `fn a_schema_4_run_and_its_resume_leave_the_checkout_and_print_no_way_back_from_it() {`

PR12-REG6-TOPOLOGY-RETURN-HINT, through the public entry points. A schema-4 run, through
`run_with`, parks on its one question; the answer is recorded through `answer::answer`; the
resume, through `resume_with`, completes the run onto its integration branch. The operator's
checkout is read before and after each: `symbolic-ref HEAD` stays `refs/heads/main` and HEAD's
commit does not move. Each summary (`RunReport::render`, which `main::finish` prints) has the line
`branch: <run branch>` and no `return with` or `git switch`, and the completed run's edit is on
that branch. The legacy control, a schema-1..3 run through the legacy writer, does check its branch
out, prints `branch: <run branch> (return with: git switch -)`, and running that `git switch -`
takes the repository back to `main`: the instruction is true where it is printed. With the render
printing the instruction unconditionally, as before repair9, the test fails at the fresh summary's
assertion, the first it reaches; the resumed summary's identical assertion is not reached then.

## `fn a_run_whose_initial_log_read_fails_after_the_census_keeps_a_shared_prefix_from_answering() {`

PR12-REG10-INITIAL-LOG-READ-IS-ABSENCE and PR12-REG8-INITIAL-LOG-READ-IS-ABSENCE (one defect),
through `answer::answer`, `answer::show` and `status::load`. Two schema-4 runs park, each on one
question; the two ids share a proper prefix (ULIDs drawn moments apart share their time digits),
and each id's prefix one character longer is its own. Both runs classify `Committed` in the census.
The first run's initial complete read then fails (`entry::initial_log_read_fault`), while the
census, which reads only the first line, still classifies it.

With that read failing, `answer` and `show` of the shared prefix are refused, each naming the
survivor's question as the one match among the runs that could be read and the first run as the
one whose log could not be read; each made exactly one attempt at the read, and the checkout
outside `.git` is unchanged: no answer file, nothing else. That comparison is asserted before the
refusals' sentences, so it is the assertion the old fold fails. `status` of the first run is
refused with the read's own error. The survivor's full id still shows its question; the first
run's full id is not found, and the refusal names the run that could not be read.

Read again, the shared prefix is ambiguous ("matches 2 questions", naming both), and each run's
own prefix shows its own question. Then a legacy run (`00LEGACY`) holds a question whose id is the
first run's own prefix followed by `LEGACY` (`L` is not a Crockford digit, so no minted id
continues it). With the first run's read failing, `answer` and `show` of that prefix are refused,
naming the legacy question as the one match among the runs that could be read and the first run as
unread, and nothing is written: the old fold dropped the run, found the topology silent and handed
the prefix to the legacy finder, which answered it. The legacy question's full id still shows it;
read again, the prefix is ambiguous between the legacy question and the first run's. Last, each
full id is answered, the survivor's and the legacy question's with the first run still unreadable
and the first run's once it reads, and the answer files are exactly those three.

Measured before commit as negative control A1 (`observe` folding the failed read into `Ok(None)`
again, as at `8b7b22d4`, over the same read): it fails at the first comparison, on "created
.upstroke/runs/<survivor>/answers" and "created .upstroke/runs/<survivor>/answers/<its
question>.json": `answer` wrote the survivor's answer to the shared prefix.
