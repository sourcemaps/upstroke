# `src/engine/tests/legacy_resume.rs`

Extended notes for [`src/engine/tests/legacy_resume.rs`](../../../../src/engine/tests/legacy_resume.rs).

## `let (_tree, repo) = temp_engine_repo("resumereviewers");`

Who judged this run is a fact about the run, not about today's
machine — step-8 finding #8's lesson on `private_dir`. Re-deriving it
would let a CLI installed since the run began become the judge for
the back half, leaving one run with two verification standards.

The work left over has to be work the rebind would OTHERWISE claim,
or this proves nothing: the task resumed onto is at frontier, where
the implementer and the reviewer are the same model.

## `let source = source(vec![Effect::NoEdit], vec![ReviewBehavior::Pass]);`

First process: no copilot on the machine, and the agent changes
nothing — so t1 exhausts its chain and parks, still unbuilt.

## `let later = cross_vendor(`

Second process: copilot has appeared since. The record still rules,
so the retry is judged by the model the run started with.

## `let resumed = resume_answering(&repo, &run_id, Effect::EditFile);`

`resume_answering` exposes only the Claude fake. If pre-flight probes
today's Copilot pin before restoring the record, this refuses before
the behavioral assertions below can run.

## `let (_tree, repo, run_id) = parked_run("resumeceilingbeforelocks");`

The same claim for the other write command, which takes two locks rather
than one. `max_per_agent = 0` is the ceiling to test a resume's ordering
with: the legacy reading below softens `max_parallel > 1` for a run that
is already sequential, but a limit with no meaning at all is refused for
fresh runs and resumes alike, so this refusal is genuinely about *when*.

## `let lease = worktree_lock_path(&repo);`

The fixture run created both lock files. Remove them, so that a file
found afterwards is evidence about this resume rather than history.

## `let competitor = WorktreeLock::acquire(&repo).expect("a competing holder takes the lease");`

And with the lease already held, so that an acquisition-first order
could only ever answer with contention.

## `fn volatile_strings(repo: &Path, run_id: &str) -> Vec<String> {`

What two runs of one fixture can never share: their run id, and the two
absolute paths their identity is built out of.

## `for path in [private_root_for(repo), repo.to_path_buf()] {`

Longest first, so a path that contains another is replaced whole.

## `fn replace_exact_runs(`

Replace every maximal run of `member` that is exactly `len` long.

Length-exact and maximal so that content-derived digests keep their meaning:
a 16-character plan hash and a 64-character normalized-plan digest are facts
two identical fixtures must agree on, and only the 40-character Git object
names and the 26-character ULIDs are unshareable.

## `fn canonicalize_json(value: &mut serde_json::Value, volatile: &[String]) {`

One JSON value with everything two runs of one fixture legitimately differ
by replaced by a token, in place.

## `if matches!(key.as_str(), "ts" | "duration_ms" | "duration") {`

Wall-clock, not meaning. Everything else — cost, usage,
effort, tier, model, reviewer, session, rung — is compared.

## `fn canonical_trace(events: &[events::Event], repo: &Path, run_id: &str) -> Vec<String> {`

One run's log as a semantic trace two runs of the same fixture can be
compared by.

Whole event bodies, not kinds. A config key that changed which reviewer
judged the retry, what effort it ran at, how long a review was allowed, or
which rung the ladder resumed on would leave the sequence of event kinds and
the run's outcome untouched — so comparing those alone would pass on exactly
the reinterpretation this exists to rule out.

## `fn canonical_projection(report: &RunReport, repo: &Path, run_id: &str) -> String {`

The run's report and every task record in it, canonicalized the same way.

`warnings` is dropped rather than compared: saying what today's config
contains is the one thing the two arms are *supposed* to differ by, and the
assertions about it are separate and explicit.

## `const LEGACY_RESUME_LIMITS: &str = "\n[engine]\nmax_parallel = 2\nmax_merge_repairs = 7\n\`

All four §17 ceilings, written the way an operator waiting for the parallel
engine would write them — `max_parallel` included, and above 1.

## `const LEGACY_RESUME_NO_LIMITS: &str = "\n# no [engine] ceilings in this arm\n";`

What the control arm appends instead.

An edit rather than nothing: §14 rolls an interrupted run's uncommitted
paths back, so an arm that left `upstroke.toml` untouched would record no
discard while the other recorded one — a difference about which fixture
edited a file, not about what the ceilings did. Both arms edit it; only one
says anything.

## `#[derive(Clone, Copy)]`

Which resume shape a legacy-limits fixture exercises.

## `Parked,`

The ordinary case: a parked run answered and carried to the end.

## `InterruptedAttempt,`

A crash prefix: the log ends inside an attempt that never settled, so
the resume has to record the interrupted settlement, refund the rung,
and retry before it can finish. This is where an unacted-on ceiling
would be most tempting to act on, because it is the only path that
re-decides how much of the ladder is left.

## `struct LegacyArm {`

One arm of a legacy-limits comparison: everything two resumes of the same
fixture must agree about, plus the warnings they are allowed to differ by.

## `trace: Vec<String>,`

Every event, whole, with the tokens two runs cannot share replaced.

## `projection: String,`

The report and its task records, canonicalized the same way.

## `tree: String,`

The tree the run committed. Content-addressed, so it is directly
comparable across two repositories whose commits can never share a sha.

## `fn legacy_resume_pair(tag: &str, fixture: LegacyFixture) -> Vec<LegacyArm> {`

Run one legacy fixture twice — once with the four ceilings in today's
config, once without — and hand back what each resume did.

## `for (fixture, tag) in [`

The keys are new; the run is not. A resume that reads all four — the one
a fresh run refuses among them — must continue exactly as it would have
without them, and say so rather than act on them.

Proved against a control resume of the identical fixture rather than by
reading the resume path: "the engine ignores these fields" is a claim
about every line of that path, and only a comparison covers all of them.

And compared *semantically*, not by event kinds and an outcome. A ceiling
that changed which reviewer judged the retry, what effort it ran at, how
long the review was allowed, which rung the ladder resumed on, or what
the attempt cost would leave the sequence of event kinds and the final
outcome identical — so a comparison that could not see those would pass
on exactly the reinterpretation it exists to rule out. Three comparisons
together close that: every event body, the report and its task records,
and the tree the run actually committed.

## `let mut open = 0i32;`

Sequential all the way through, stated as a property of the log
rather than of the outcome: one attempt open at a time is what
`max_parallel = 2` would have changed if anything acted on it.

## `EventBody::AttemptFinished { .. } | EventBody::AttemptInterrupted { .. } => {`

Both settlements close one: a crashed attempt's recovery
record is as much an end as a finished one.

## `for key in [`

Every one of the four is named, and only where it was written.

## `let (_tree, repo) = temp_engine_repo("oldlogresume");`

`RunStarted.reviews` is #[serde(default)] so a step-8 log still
parses — but the default is an EMPTY plan, which every later reader
cannot tell apart from `review = { enabled = false }`.

## `let paths = paths_of(&repo, &first.run_id);`

Rewrite run_started as a pre-step-9 process would have written it.

## `let later = source(vec![Effect::EditFile], vec![ReviewBehavior::Fail]);`

A reviewer that rejects everything: if review still runs, nothing can
commit. If the absent field read as "review disabled", it commits —
verification gone without a word, which is step-6 finding #10.

## `struct Scenario {`

One path through the ladder, for the live-equals-replay property.

## `plan: Option<&'static str>,`

Overrides the default two-task plan where a scenario needs path
hints or a particular tier.

## `second_opinion: Option<Vec<ReviewBehavior>>,`

`Some` puts a second vendor on the machine (§11.3).

## `let scenarios = vec![`

One scenario per branch the engine can take, so the equality is
exercised against commits, retries, escalations, deferrals, parks,
answers, and a halt — not just the happy path.

## `Scenario::new(`

§11.3: two review passes per attempt, so `AttemptRecord.reviews`
carries more than one entry through serialize → deserialize. The
list replaced a scalar pair in step 9; this is what proves the
new shape survives the wire.

## `Scenario::new(`

And the same with the second reviewer rejecting, so a `false`
verdict on a non-final pass replays too.

## `Scenario::new(`

The anti-self-review rebind: the acceptance pass runs on a model
no chain rung names, so the record has to carry the binding
rather than let a replay re-derive it.

## `Scenario::new(`

Step 10's two new branches. `budget_exceeded` folds into a
run-level field and `capacity_snapshot` folds into nothing —
opposite shapes, and both have to come back the same on replay.

## `Scenario::new(`

And the ApproveSpend park, whose fold depends on the escalation
having landed *before* the park — the ordering D3 turns on.

## `if cross_vendor_scenario {`

A cross-vendor scenario that quietly resolved to one pass would
still replay identically and prove nothing about the shape this
step introduced. Check the run did what the scenario claims
before trusting the equality below.

## `let (_tree, repo) = temp_engine_repo("abortlog");`

The engine dying between the agent's edits and a verdict is §19's
"engine crash" row. Nothing gets to write a tidy ending, so the log
has to be enough on its own.

## `let source = source(vec![Effect::EditFile], vec![ReviewBehavior::Pass]);`

Run it through, then rewind the log by hand: reproducing a real
abort at exactly this point needs a failure the fake adapter cannot
raise, and the on-disk shape is what this test is actually about.

## `let paths = paths_of(&repo, &report.run_id);`

Now truncate the log to the moment before the attempt reported, the
exact on-disk shape a kill leaves, and confirm it still folds.

## `let nothing: [f64; 0] = [];`

Observed as `total: $-0.0000` in the ledger of a run whose first
attempt was still in flight. This first assertion is the diagnosis,
kept because it is the whole reason `total_of` exists: if std ever
folds from `+0.0`, the helper can go.

## `let spent = vec![`

And the fold change cannot have moved a real total: `+0.0` preserves
every value a cost can be.

## `let (_tree, repo) = temp_engine_repo("livestatus");`

The settlement above, inverted. A run an engine is still driving has
a dangling attempt at every instant, exactly like a killed one — so
settling unconditionally reports a working attempt as a failure and
the whole run as halted. `status` is the only window into a run that
holds its own terminal, and a window that lies is worse than none.

## `let text = fs::read_to_string(paths.events()).expect("log");`

Rewind to mid-attempt: the shape a live engine's log has the whole
time it is working, not only the shape a kill leaves behind.

## `let stopped = replay_of(&repo, &report.run_id);`

With nothing holding the run, that shape still means interrupted —
and `t2` really is blocked, because on an ended run a dependency that
never finished never will.

## `let lock = RunLock::acquire(&paths.public).expect("simulate a live engine");`

Now hold the lock the way a working engine does — through the same
`RunLock` a run takes, not a hand-rolled `flock` on the same path.
Which primitive holds a run is `rundir`'s to decide, and a test that
reaches around it is testing a lock nothing else uses.

## `assert!(out.contains("t2: queued"), "{out}");`

The one the dependency-free pair could not catch: `t2` is waiting on
a task that is working, which is what `Queued` means. Reading that as
`Blocked` tells the operator a dependency failed when it is running.

## `let (_tree, repo) = temp_engine_repo("resumetrunc");`

Decision 3, end to end: the attempt shows up in the ledger, the
rung's allowance does not, and the task completes on the retry.

## `Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),`

One attempt on one rung: if the interrupted attempt had been
counted, the task could never commit.

## `let text = fs::read_to_string(paths.events()).expect("log");`

Rewind the record to mid-attempt and put the tree back the way a
dead agent would have left it.

## `assert!(`

The residue is gone and the branch is linear.

## `let (_tree, repo) = temp_engine_repo("crashkill");`

The real thing: a separate process is driven into an attempt and
dies inside it, exactly as `kill -9` or a power cut would.

## `assert!(`

What a kill leaves: a dirty tree and an attempt that never reported.

## `assert!(`

The lock died with the process, so nothing has to be cleared by hand.

## `let rendered = crate::status::render(&before);`

And the summary line says what happened rather than claiming an
outcome. A killed run replays into `Complete` — nothing halted it, no
budget stopped it, nothing is parked — so the ledger used to be
followed by `run complete: 1 task(s) committed` and then, one line
later, `state: interrupted`. Two adjacent lines contradicting each
other about a run that died mid-attempt with work left undone.

## `assert!(rendered.contains("skipped (run interrupted)"), "{rendered}");`

Its unreached tasks were not skipped because the run *halted* — that
is a different ending, and one an operator acts on differently.

## `let (_tree, repo, run_id) = parked_run("headmoved");`

§15's HEAD check. Something committed after the run stopped, so the
log no longer describes what is on the branch.

## `let (_tree, repo, run_id) = parked_run("chainmoved");`

`Progress.rung` is an index into the chain; re-resolving a different
chain would point it at another tier without saying so.

## `fn parked_run_with_gate(tag: &str, cmd: &str) -> (ScratchTree, PathBuf, String) {`

[`parked_run`], with one `[[gates]]` entry — the resumable shape for the
gate tests, which need a recorded gate to diverge from.

## `fn gate_config(cmd: &str) -> String {`

[`PARKED_RUN_CONFIG`] plus a `check` gate running `cmd`.

## `let (_tree, repo, run_id) = parked_run_with_gate("gaterecorded", "git --version");`

The load-bearing test for the whole gate record, and behavioural
rather than textual: the recorded gate passes, today's config would
fail, and the task commits — which it can only do if the gate that
actually executed came from the log.

This is the self-hosting hazard from the gate-config record, closed
at the point
that matters. The workspace an implementer edits contains the very
upstroke.toml its gates come from, so an edited gate must not become
the standard for what follows. Refusing would also have stopped the
weakened gate running, but it would have stopped the *run* too, and
a legitimately-committed config edit would have left it unresumable.

## `fs::write(`

`git` still resolves at pre-flight, so nothing refuses before the
gate runs — it just exits non-zero when it does.

## `let warning = resumed`

And the operator learns their edit did not take effect here, rather
than concluding the gate is broken when it never ran.

## `assert_eq!(resumed.gates, ["check"]);`

The report describes the gates that ran, not the ones on disk.

## `let (_tree, repo, run_id) = parked_run_with_gate("gate_record_lenient", "git --version");`

`design/15`: gates are taken from the record, not re-derived — and not
refused over. Today's file has changed since the run started, and
changed into shapes a fresh run refuses: a zero timeout, a second
entry repeating the first's name, and a key no entry has. The record
decides what runs, so the resume goes through, says what it saw, and
runs the recorded gate — which is the only way `t1` can commit.

This is the composition the config-level tests cannot see: which
reading `resume.rs` chooses from the log, and that the record is what
then executes. Pass 2 on PR #150 showed a parser-only downgrade wrong
in both directions; this test and its twin below are the two
directions.

## `let (_tree, repo, run_id) = parked_run_with_gate("gate_record_absent", "git --version");`

The other direction. A log from before the gate record has nothing to
substitute: this resume settles the run's gates from today's file, so
today's file governs, and `timeout_sec = 3600` is a gate that would
really run at the 600 s default — the harm a warning cannot prevent.
So the resume refuses, before any effect, naming the key.

## `let after = fs::read_to_string(paths.events()).expect("the log after");`

Refused before any effect: the log is byte-identical to what it was.

## `fs::write(repo.join("upstroke.toml"), gate_config("git --version")).expect("edit config");`

And with the key spelt right the same log resumes and, having no
record, settles today's gate and runs it.

## `let (_tree, repo, run_id) = parked_run_with_gate("gatelabel", "git --version");`

`gates` came from the record but `gates_from_config` did not, so the
run's own report and a later `status` disagreed about the same list:
`finish()` read today's analysis while `RunReport::from_state` read
the record. The doc above `from_state` promises those two cannot
drift, and this is the one field that still let them.

## `fs::write(repo.join("upstroke.toml"), PARKED_RUN_CONFIG).expect("edit config");`

`[[gates]]` deleted, so today's flag would be false and today's
derivation empty — the temp repo has no project marker.

## `let replayed = replay_of(&repo, &run_id).report();`

The other half of the same promise: a reader replaying the log agrees.

## `let (_tree, repo, run_id) = parked_run_with_gate("gateunmoved", "git --version");`

The success path, with a non-empty gate list — the direction a false
positive would break. Every other gate test edits the config, so
without this one an over-eager comparison (order, whitespace, a
re-derived timeout) would warn on every ordinary resume unnoticed.

## `let gate = |name: &str, cmd: &str| GateSummary {`

`[[gates]]` does not require unique names, so the obvious by-name
lookup answers for the wrong entry: it reports edits nobody made,
and — worse — finds every name present and concludes "reordered"
when a gate was added. Each case here produced a false sentence
before the comparison paired whole gates instead of names.

## `let added =`

A duplicate name added. The record's `check` is present and unchanged,
so nothing was edited and nothing reordered — one gate appeared.

## `let removed = gates_differ(&[check.clone(), gate("check", "cargo clippy")], only_check)`

One of two same-named gates removed. Pairing by name would report
`check` as edited from one command to the other; both are real
entries and neither changed.

## `let edited = gates_differ(only_check, &[gate("check", "true")]).expect("a difference");`

An unambiguous single-name edit still reads as one edit.

## `let renamed = gates_differ(only_check, &[gate("verify", "cargo test")]).expect("a difference");`

A rename is two facts, and saying so beats guessing which gate the
operator meant to rename into which.

## `let reshelled = gates_differ(`

Shell and timeout are recorded because they decide what a command
means and how long it has to mean it — `true` always passes under sh
and is not a program at all under cmd.

## `let other = gate("test", "cargo test");`

Same gates, different order: a difference worth a line, but not the
same claim as a changed command.

## `let (_tree, repo, run_id) = parked_run_with_gate("oldgatelog", "git --version");`

A v0.1 log recorded gate names and nothing else. Refusing would
strand every run written before the record over a field it could
never have carried, so resume re-derives — and uses the one thing
such a log *does* have. A moved name is proof the standard changed,
not a suspicion, and the warning says which.

## `fs::write(`

Re-derivation must be a real re-derivation, or this test would pass
against a resume that ignored today's config entirely.

## `let (_tree, repo, run_id) = parked_run_with_gate("oldgateestablish", "git --version");`

Without this, the pre-record population never gains a record: every
resume re-derives, so a gate weakened between two of them is adopted
silently — the exact substitution the record exists to prevent,
surviving in the one population that could not carry it.

Behavioural, and it takes two resumes to show: the first establishes
`git --version`, the gate is then weakened to something that fails,
and the second must still commit. It can only do that by running the
gate the first resume wrote down.

## `let first = resume_answering(&repo, &run_id, Effect::NoEdit);`

First resume: nothing to rebuild from, so it re-derives and says so.
`Effect::NoEdit` leaves the task parked, so there is a second resume
to make.

## `let paths = paths_of(&repo, &run_id);`

It wrote down what it settled on.

## `fs::write(`

Now weaken the gate, exactly as an implementer editing the workspace
would. Under the old behaviour the second resume re-derived and
adopted this.

## `assert!(`

And it is an ordinary record-bearing resume now: it warns about the
difference rather than about the log's age.

## `let (_tree, repo, run_id) = parked_run("oldgatelessslog");`

The run recorded no gates and none resolve today, so no command can
have hidden behind an unchanged name. A warning here would fire on
every gateless pre-record run, and one that cries wolf on the
harmless case is not read on the harmful one.

## `#[test]`

---- step 8: status and the ledger -------------------------------------

## `rendered.contains("per-pool drain: no pool is connected"),`

No pools file in these tests, so no attempt names a pool — and
the ledger says exactly that rather than showing a blank column
that reads as "nothing was spent".

## `assert!(`

The ledger totals are the run's, derived from the log rather than
carried over from the process that wrote it.

## `let paths = paths_of(&repo, &report.run_id);`

Holding the lock on a run that has already recorded its finish does
not make it live again. It says a process has claimed the run —
which is what a `resume` looks like before it writes anything — and
leaves the outcome above alone. A live run is covered by
`a_live_run_reads_as_running_rather_than_halted`, which truncates the
log so that the run genuinely has somewhere left to go.

## `let (_tree, repo) = temp_engine_repo("private");`

The §15 split, and the reason the private root cannot be inside the
repo: §14's rollback is `git clean -fd`, which would delete it.

## `let adapters = source(`

The first attempt fails, so a rollback happens before the second.

## `assert!(paths.events().starts_with(&repo));`

The ops surface stays where §15 documents it.

## `let in_repo = repo.join(".upstroke").join("runs").join(&report.run_id);`

And nothing agent-authored is reachable from the repo.

## `fn strip_run_started_field(paths: &RunPaths, field: &str) {`

---- step 8.1: the seams either side of the log ------------------------
Drop a field from a log's `run_started` — the shape a log written before
that field existed has.

Selects the event by its tag rather than by line number: `run_started`
is first today, and a helper that hard-codes that would silently rewrite
an unrelated event the day something precedes it.

## `fn rewrite_run_started_as_schema_one(paths: &RunPaths, absent: &[&str]) {`

Rewrite the opening event into the exact compatibility shape a
schema-1 binary wrote: selected top-level fields absent and no per-chain
binding snapshot. Used only by downgrade/resume regressions.

## `fn truncate_log_before(paths: &RunPaths, event: &str) {`

Rewind a log to just before the named event — the shape a process
killed at that instant leaves behind.

## `let (_tree, repo) = temp_engine_repo("adoptcommit");`

§14 commits, reads the sha back, scrubs the tree, and only then
appends `task_committed`. A process killed inside those three git
calls leaves the branch one commit past its own log — which is what
foreign history looks like too. Refusing would tell the operator to
reset away a commit that already passed its gates and its review,
and to spend the attempt a second time.

## `truncate_log_before(&paths, "task_committed");`

The commit is on the branch; the log stops just short of it.

## `let question_id = QuestionId::from("q-before-success");`

Model an earlier answered question whose DesignDefect append was
interrupted. It is unrelated to the later successful settlement,
but resume still owes the repair after closing that settlement.

## `let (_tree, repo) = temp_engine_repo("adoptforeign");`

Exact object identity, not a plausible subject, is the authority.

## `let (_tree, repo) = temp_engine_repo("privatedir");`

Which private root a run used is a fact about that run. Recomputing
it from today's environment — another HOME, a service account, the
no-home fallback — would scatter the rest of its transcripts
somewhere `status` never looks.

## `truncate_log_before(&recorded, "attempt_finished");`

Stop before the successful settlement, so this is a genuinely
interrupted attempt rather than a settled prepared commit whose pin
a real process would still retain.

## `let mut resume = resume_options(&repo, &run_id);`

No override, so the resume has to read the location off the record.

## `let (_tree, repo) = temp_engine_repo("stalepayload");`

The engine emits `question_answered` and then rewrites the payload
beside it. A crash in between leaves a file that still reads as
open, though the log has already closed the question. `upstroke
answer` still refuses a second answer against it as already answered,
because the first answer's published file is still there and
publication is create-once; the test asserts that refusal before the
resume makes the payload agree with the log.

## `let retry = source(vec![Effect::NoEdit], vec![ReviewBehavior::Pass]);`

The retry fails the same way, so the run ends parked on a *second*
question with the first one answered in the log.

## `let questions = rundir::public_dir(&repo, &run_id).join("questions");`

Rewind the payload to what a crash mid-ingest leaves.

## `let (_tree, repo) = temp_engine_repo("husk");`

Nothing is on the record until the first event lands. A failure in
that window would otherwise leave a run directory with no
`events.jsonl`, and since run ids sort newest-last it becomes what a
bare `upstroke status` reports on — "no event log here" for a run that
never began, shadowing the real latest one.

## `git_in(&repo, &["branch", "upstroke"]);`

Git stores refs as paths, so a branch literally named `upstroke` is a
file where `upstroke/run-<id>` needs a directory: branch creation
cannot succeed.

## `struct LockReleasingSleeper {`

Holds a run's lock and lets go after a set number of sleeps — an engine
that finishes while a follower is waiting on it.

## `let (_tree, repo) = temp_engine_repo("followlive");`

A whole attempt — the agent's thinking, its tool calls, the gates,
the review — folds into one `attempt_finished`, so a healthy run
says nothing for minutes at a time. The idle budget exists to
release a terminal attached to a dead engine; spending it on a live
one drops the operator's view mid-run.

## `let text = fs::read_to_string(paths.events()).expect("log");`

Drop the ending, so `follow` idles rather than stopping at it.

## `crate::status::follow(&loaded, &sleeper, Duration::ZERO, 1, &mut out).expect("follow");`

A budget of one idle poll: without the liveness check this returns
after two sleeps, whatever the run is doing.
