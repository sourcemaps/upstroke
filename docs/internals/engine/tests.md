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

## `#[derive(Default)]`

---- a returned legacy append error ------------------------------------
Fails the **third** legacy append of a live run, by returning an error at
`Event.LegacyAppend`'s `Written` point.

The third, and the number is load-bearing rather than arbitrary:
`run_harness_inner_on` emits `run_started` and then the capacity snapshot —
two appends — *before* `drain_and_report` is called at all, so a fault at
either tests the startup path and never reaches the branch both findings are
about. The third is the first append inside `drain()`, which is where
`production_effect`'s "it reports and stops" and the partial report live.
`a_returned_legacy_append_error_still_leaves_the_partial_report` checks that
the two startup appends really did land, so if this number ever stops being
the right one it fails loudly instead of passing for the wrong reason.

## `#[test]`

A returned append error **stops the run** — it is not swallowed and carried
on from (`PR5-CONF-010`).

`production_effect` says "the legacy engine's handling of a returned append
error is unchanged — **it reports and stops**". The shipped code did;
nothing required it to. Replacing `Run::emit`'s `?` with an arm that pushed
a warning and returned `Ok` **survived the whole suite**: every append
failure the suite injected targeted an `EventLog` a test had built directly,
and `emit` reached `EventLog::append`, which hard-codes `NoEventHooks`, so
no fixture could make a **live `Run`**'s append fail at all.

The two axes this crosses are *whose* `EventLog` fails and *what observes
the failure*. `src/events/log/tests.rs` holds the first constant at "a log
the test owns" and varies the second exhaustively; the census beside those
tests reads the coordinator's source and can see that the branch returns,
but not that the error ever gets to it. What varies here is the log: it is
the live run's own, reached through `engine::run_with`, and the assertion is
on the value the *caller* receives.

## `#[test]`

…and the partial report is written beside the log on the way out
(`PR5-CONF-011`).

Deleting `drain_and_report`'s partial `finish()` and `rundir::write_report`
survived the whole suite, for the same reason and one branch further on.

**This is the legacy path and the repair must not generalize.**
`coordinator_integration.append_error_protocol` forbids exactly the
opposite for schema-4 — "no report, status, question payload, or cleanup is
derived from the poisoned fold" and "still performs no retry, **report from
memory**, cleanup, or fold mutation". So the assertion below is on the
legacy `report.json` only, and nothing here is asserted of the topology
coordinator.

Held constant with the sibling above: the same fault, at the same append, in
the same fixture. What varies is what is examined — the caller's return
value there, the run directory here.

## `let runs = opts.repo_root.join(".upstroke").join("runs");`

The run directory the failed run created, found by its own log rather
than by a run id the caller never received.

## `let log = fs::read_to_string(public.join("events.jsonl")).expect("the log");`

The premise: the fault fired *inside* `drain()`, not during startup.
`run_started` and the capacity snapshot are appends 1 and 2 and both must
have landed whole; the third is the one that failed, and its Written
error-return arm leaves a torn prefix rather than a complete line.

## `#[test]`

---- step 1-6 behaviour, unchanged by the ladder ----------------------

## `assert!(`

Per task: implementer 0.01 + reviewer 0.05 (§11.2 reviews every
attempt), so both spends are accounted for.

## `let paths = paths_of(&repo, &report.run_id);`

Rewind to the exact atomic settlement, then add dirty residue to
model death before ordinary post-attempt cleanup. Replay must retain
both the paid ledger line and the question, discard the residue, and
never dispatch another worker for the known-oversized identity.

## `Some("[routing]\ntest = { chain = [\"small\"], attempts_per = 1 }\n"),`

One rung, one attempt: the provenance failure is what is under
test, not the ladder's reaction to it.

## `seed(`

A gate that creates a file: residue must never reach a commit nor
survive the task.

## `assert!(`

§15 split: the file describing an agent's own sandbox is not
somewhere that agent can read.

## `let source = source(vec![Effect::EditFile], vec![ReviewBehavior::Fail]);`

A reviewer that would REJECT everything: if review still ran, the
task would never commit.

## `const FRONTIER_AUTH_PLAN: &str = "## Rotate the signing key\n\`

---- step 9: cross-vendor review (§11.3) -----------------------------
A plan whose one task runs at frontier and touches `src/auth/**`, so
both step-9 mechanisms are in play: its implementer binds to the same
model as the reviewer, and its paths can match a `second_opinion`
override.

## `const FRONTIER_ONLY_CONFIG: &str =`

Same task, no override — the implicit anti-self-review path.

## `let (_tree, repo) = temp_engine_repo("secondopinion");`

§11.3: both verdicts must pass. And the primary must NOT rebind here
even though it matches the implementer — rebinding would resolve both
passes to copilot/gpt-5.3-codex and drop the Anthropic review entirely, which
is worse than the self-review the rebind exists to prevent.

## `assert_eq!(t1.review_cost_usd, Some(0.10), "0.05 per pass");`

Both reviewers' spend lands in the review column, not the worker's.

## `let (_tree, repo) = temp_engine_repo("secondopinionfail");`

The point of two passes: the one that says no decides, even when the
first already approved.

## `let (_tree, repo) = temp_engine_repo("shortcircuit");`

Passes short-circuit like gates do (§11.1): once one has said no, a
second opinion on the same diff changes nothing and costs a frontier
invocation to learn it.

## `let (_tree, repo) = temp_engine_repo("selfreview");`

The item carried since step 6: both binders resolve `frontier`
identically, so without the rebind the reviewer IS the implementer.

## `let source = cross_vendor(`

The claude adapter's review script says FAIL and the copilot one says
PASS, so a committed task proves which of them was actually asked.

## `let (_tree, repo) = temp_engine_repo("noneedtorebind");`

A mid-tier implementer judged by the frontier reviewer is already a
genuine second look, so nothing rebinds. Triggering on family
similarity instead of exact identity would send most of a run
cross-vendor for no verification gain.

## `let (_tree, repo) = temp_engine_repo("nosecondfamily");`

Step-6 finding #10's posture: the operator asked for two model
families on their blast-radius paths. Quietly giving them one is the
failure that finding exists to prevent, so this refuses instead.

## `let (_tree, repo) = temp_engine_repo("selfreviewwarn");`

The implicit rebind is upstroke's own idea, not the operator's, so a
single-vendor machine loses the upgrade rather than the run — but it
is told, because a verification property that quietly is not there is
exactly what step 6 objected to.

## `let (_tree, repo) = temp_engine_repo("brokencopilot");`

Installed but broken is different from absent, and the two probe
classes have to agree about which is which: the opportunistic
reviewer only warns.

## `assert!(`

And it names the tasks. Resolution cannot reach this warning — a
shipped binary always has the Copilot adapter, so the only way the
rebind really goes missing is a probe failure, and a warning that
never fires for a real user is not a warning.

## `let (_tree, repo) = temp_engine_repo("brokenrequired");`

Same machine, same breakage — but now a `second_opinion` names it, so
it is load-bearing rather than opportunistic.

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

## `let (_tree, repo) = temp_engine_repo("maxparallelrefusal");`

The config refusal is only worth having if it lands before the run has
done anything an operator must undo. Pre-flight loads the config ahead of
the run id, the run directory, the run lock, and the branch — so a
ceiling this engine cannot honour leaves the repository exactly as it
was, rather than a husk under `.upstroke/runs` that `latest_run` then
reports on in place of the real one.

## `fn worktree_lock_path(repo: &Path) -> PathBuf {`

Where this repository's worktree lease file would be, held or not.

## `let (_tree, repo) = temp_engine_repo("ceilingbeforelease");`

The ordering claim itself, tested where cleanup cannot fake it.

A run that took the lease, *then* read the config, then tidied up on its
way out would leave a repository indistinguishable from this one — so an
end-state assertion proves nothing about when the config was read. These
two do:

 (a) The lock file is created by acquisition and is never removed by
     release, by design (a killed engine must leave nothing to clear by
     hand, and the OS releases the hold; the file stays). Its absence is
     therefore proof that acquisition never happened, not proof that it
     was undone.

 (b) With a competing holder already on the lease, an acquisition-first
     order cannot produce a config error at all: the acquisition is what
     fails, and the operator is told another process owns the worktree —
     the wrong diagnosis for a file they can fix in five seconds. Move
     `validate_inputs` back below `WorktreeLock::acquire_in` and this
     half fails no matter what any cleanup does afterwards.

## `let source = fake(Effect::EditFile);`

(a) Uncontended: the refusal must not have created the lease file.

## `let competitor = WorktreeLock::acquire(&repo).expect("a competing holder takes the lease");`

(b) Contended: the config error must still be the one that comes back.

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

## `fn config_with_repairs(repairs: u32) -> String {`

A config with a distinguishable, harmless ceiling in it.

`max_merge_repairs` is the right knob for the tests below: it is kept
verbatim by every reading, it loads without refusing, and its value is
visible on the `Analysis` — so "which bytes produced this analysis" has a
direct answer rather than an inferred one.

## `let (_tree, repo) = temp_engine_repo("confirmunderlease");`

The pre-lock check answers "may this start", from files the worktree did
not yet belong to this run. Adopting *that* analysis afterwards would
execute an answer about bytes that no longer exist, so what the lease
holder adopts is an analysis it captured and validated itself — on the
condition that the two captures agree about what it was reading.

## `fs::write(&config, config_with_repairs(5)).expect("the config before the lease");`

(a) A change that is still there at the lease is adopted, not papered
    over with the pre-lock reading. Nothing here is refused: the point is
    that a stale-adopting implementation returns 5 and a re-validating
    one returns 7, and only one of those is the config the run will hold.

## `fs::write(&config, config_with_repairs(5)).expect("the config before the lease");`

(b) And if the change is one this engine must refuse, it refuses — under
    the lease rather than never. A run whose config was checked in an
    earlier life and replaced since is a run executing something nothing
    ever validated, which is the whole defect.

## `fs::write(&config, config_with_repairs(5)).expect("A");`

(c) The A-to-B-to-A interleaving, end to end. The excursion is invisible
    to both captures — which is precisely why the analysis may not come
    from a read taken beside them. It comes from the capture itself, so
    what is adopted is A whether or not B ever existed.

## `let (_tree, repo) = temp_engine_repo("gatesunderlease");`

The one input `analyze` still reads from the filesystem rather than out
of the capture: `gates::derive` is handed a directory. So the derivation
has to happen where the worktree is this run's — which means the adopted
analysis cannot be the pre-lock one, and the files it looks at have to be
in the captured set so that a change to them is a change this confirmation
notices.

## `fs::write(repo.join("Cargo.toml"), "[package]\nname = \"x\"\n").expect("a rust repo now");`

The repo becomes a Rust repo between the check and the lease.

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

## `let (_tree, repo) = temp_engine_repo("outagerecord");`

Step-6 finding #8's distinction, carried into the ledger: a judge
that never ran said nothing about the code, and recording it as a
plain "did not pass" puts a rejection against a model that never read
the diff.

## `assert!(committed(&report, "t1"), "{report:?}");`

And the ladder treated it as an outage: deferred, then committed.

## `let (_tree, repo) = temp_engine_repo("partialcost");`

The Copilot route bills nothing back (§13), so a two-pass review
shows one reviewer's spend. Presenting that as the total is exactly
what `render_ledger` says is worse than no ledger at all.

## `let (_tree, repo) = temp_engine_repo("reviewtrail");`

An escalated task can be judged on one rung by one model and on the
next by another. `review_cost_usd` sums every attempt, so a list
scoped to the final attempt would read as though it explained a total
it does not cover.

## `let source = cross_vendor(`

Mid fails review, escalates to frontier, which passes. The frontier
rung is self-review, so its pass rebinds to the other family.

## `let (_tree, repo) = temp_engine_repo("passtranscripts");`

Two reviewers, two records. The acceptance pass keeps the bare name
it has had since step 6, so a run directory reads the same way
whether or not a second opinion was configured.

## `assert!(`

A clean pass still passes.

## `let failure = review_failure(review::ReviewResult::Unavailable {`

A rate-limited or hung judge must not read as "your code is wrong",
or the ladder retries the implementer for an outage.

## `let prompt = materialize_prompt(`

Missing input: say so plainly rather than pointing at nothing.

## `fs::write(`

Present input: content is inlined.

## `#[test]`

---- step 7: the ladder in the engine ---------------------------------

## `let (_tree, repo) = temp_engine_repo("resume");`

§21 definition-of-done (b). The gate demands a file only the second
attempt writes, so recovery is real rather than scripted around.

## `let files = git_in(&repo, &["show", "--name-only", "--format=", "HEAD"]);`

§14: a resumed retry keeps the tree, so the commit carries BOTH
attempts' work rather than only the last one's.

## `let (_tree, repo) = temp_engine_repo("escalate");`

§21 definition-of-done (c).

## `assert_eq!(runs[0].model, "claude-haiku-4-5");`

The adapter's own record, not just the report echoing what the
engine intended: the second attempt really was dispatched to the
higher rung's model.

## `let (_tree, repo) = temp_engine_repo("park");`

§21 definition-of-done (d) and invariant 6: t1 exhausts its chain
and parks; the independent t3 must still commit.

## `let source = source(`

t1 fails; every later attempt (t3's) edits and passes.

## `let path = repo`

The question is on disk where a notifier, `upstroke answer`, or a UI
can read it — that file is the contract, not the terminal output.

## `Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),`

A single attempt on a single rung: if the rate limit spent it,
the task could never commit.

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

## `let reviews = paths_of(&repo, &report.run_id).reviews();`

The re-ask actually happened, and both sides are on record.

## `fn raw_object_after(line: &str, key: &str) -> Option<String> {`

The raw text of the JSON object that follows `key` in `line`.

Byte-exact and order-preserving, which is the whole point: a `serde_json::Value`
round-trip re-emits an object with its keys sorted, and a claim about *what the
log holds* cannot be made from a value that has been through one. Tracks string
state so a brace inside a gate's error text does not end the object early.

## `#[test]`

**The legacy record did not change when schema 4's did.**

`FailureRecord::detail` exists for schema 4, whose settlement transitions have no
feedback field. The record builder is shared with `coordinator.rs`, and the first
version of the repair copied the value in unconditionally — so the **live**
`upstroke run` path began writing an 8-KiB gate tail onto the legacy wire and into
`report.json`, once per failed attempt, duplicating the `ladder_retry` copy that
already held it. The 2026-08-26 re-review of `c2c0294` found it as finding A.
`classify::FeedbackCarrier` is the repair; this is what holds it.

**The fixture is real, and it is compared byte for byte.** `PRE_CHANGE_FAILURE` below
is the exact `"failure"` object `610106b` — the commit before the field existed — wrote
for this scenario, captured by running it there. The earlier version of this test
quoted it *elided* in prose and then asserted three key names and
`reason.starts_with(...)`, so a changed reason suffix still passed. The round-3 review
of `bf927f3` said so, and it was right: the body and the decision record both claimed a
byte comparison this test did not make.

**One
residual difference that is stated rather than hidden**: `detail` serializes as an
explicit null, because `skip_serializing_if` is not available here. Schema 4's strict
door decides "unknown field" by asking the record which keys it claims back, so a
field that decodes from `"detail":null` and re-encodes to nothing makes the door
refuse every failed attempt's settlement — measured, and held by
`an_explicit_null_detail_survives_the_strict_door` above.

So the claim is exact: **strip `,"detail":null` and the bytes are the pre-change
bytes.** Not "similar", and not asserted by rebuilding the expected value from the
observed one — the strip must fire, and what is left must match key for key.

## `const PRE_CHANGE_FAILURE: &str = concat!(`

The exact bytes `610106b` wrote for this scenario's failed attempt. Captured by
building a worktree at that commit and running the same gate; not derived from
anything this build produces.

## `let bytes =`

**The log's own bytes, sliced out of the line.** Not
`serde_json::to_string(failure)`: a `Value` round-trip sorts keys, so
the field order the comparison is about would be the map's rather than
the struct's, and the pre-change fixture's `,"detail":null` suffix
would never appear where it actually appears.

## `assert_eq!(`

**Byte for byte against the captured fixture.** Not a key-set check and not a
`starts_with`: every byte of `kind`, `origin` and `reason` must be what
`610106b` wrote, so a changed reason — a different gate name, a reworded
prefix, an extra suffix — fails here instead of passing a prefix predicate.

## `let object = failure.as_object().expect("the failure is an object");`

The point of the whole finding: the tail is not on the record. Asserted as an
**explicit null and not merely a falsy read** — `failure["detail"].is_null()`
answers true for an absent key too, so it could not tell "the field is here
and empty" from "the field is gone", which are different wires.

## `if event`

And it is still delivered — by the carrier that owns it here.

## `let report: serde_json::Value = serde_json::from_str(`

`report.json` is the other reader of these records, and the reason
`LadderRetry` holds the text instead: it "should not grow one per attempt".

## `let runs = source.adapter.runs();`

The retry was still told. Delivery is the whole reason the field exists on the
other engine, and removing it here must not cost the legacy one anything.

## `#[test]`

**The strict door's precondition, checked over the record that gained a field.**

Schema 4 refuses an unknown field in a transaction payload (`refusals[24]`) by
decoding a record, re-encoding it, and reporting any key the input carried that
the record did not claim back. That is exact **only** while every embedded record
serializes each field it deserializes — no `skip_serializing_if` — and
`topology::events::strict`'s own documentation says so.

`events::tests::a_known_null_survives_the_strict_door_and_an_unknown_null_does_not`
checks that precondition, and checks it over `AttemptRecord`'s own optional fields:
its fixture has `failure: None`, so **no `FailureRecord` appears in the payload at
all**. Adding `skip_serializing_if` to `FailureRecord::detail` therefore leaves that
test green while breaking the door for every failed attempt — measured 2026-08-26,
which is why `decisions/2026-08-26-durable-retry-feedback.md`'s argument for a plain
`#[serde(default)]` needed a witness rather than a paragraph.

This is that witness, one record deeper. It lives here because
`src/topology/events.rs` is frozen and this needs no change to it.

## `failure.insert("detail".to_owned(), serde_json::Value::Null);`

Explicit, not absent: an absent key is the older-log case and is covered by
`a_log_predating_the_detail_field_folds_and_resumes`. What is under test here
is the key being *present* and null, which is what this build writes.

## `#[test]`

**Both of §11.4's feedback sources reach the durable record.**

§11.4 names exactly two: "failure feedback (gate log or `required_changes`)
goes back to the *same rung*". `AttemptFailure::feedback` unifies them —
`classify::gate_failure` puts the 8-KiB tail there, and
`attempt::review_failure` puts the reviewer's required changes there — and
`classify::attempt_record` is the one production construction that copies
the pair onto the wire.

It copied `{kind, origin, reason}` and dropped the feedback. `reason` is the
human-facing summary, so a resumed run could say *that* a gate failed and
nothing about **what it printed**: the retry ran on attempt 1's prompt and
could repeat the same defect while spending another attempt. The 2026-08-26
frontier review of `75da796` raised it as finding 2 and it is
`PR7-FEEDBACK-NOT-DURABLE-IN-SCHEMA-4`; the field is authorised by
`decisions/2026-08-26-durable-retry-feedback.md`.

Asserted on **content**, not on `is_some()`: the claim is that the next
worker is told what this one was told to fix, and a record carrying the
wrong string satisfies a presence check exactly as well as the right one.

## `fn durable_detail(failure: &crate::ladder::AttemptFailure) -> Option<String> {`

One failure's `detail` as the wire carries it, through the production
builder rather than around it.

## `feedback: super::classify::FeedbackCarrier::AttemptRecord,`

The schema-4 carrier, because that is the path under test: this
helper exists to assert the field is written at all.

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

## `fn replay_of(repo: &Path, run_id: &str) -> crate::status::RunStatus {`

---- step 8: the event log is the state ------------------------------
Fold a run's log the way `status` and `resume` do.

## `struct Scenario {`

One path through the ladder, for the live-equals-replay property.

## `plan: Option<&'static str>,`

Overrides the default two-task plan where a scenario needs path
hints or a particular tier.

## `second_opinion: Option<Vec<ReviewBehavior>>,`

`Some` puts a second vendor on the machine (§11.3).

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

## `fn task_report_costing(worker: Option<f64>, review: Option<f64>) -> TaskReport {`

A report carrying nothing but the two cost columns.

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

## `fn the_v1_conductor_runs_and_resumes_on_the_graph_its_own_workspace_wrote()` › `run_replacement_witness_child`

The helper runs in a child because what it measures reads the environment
this process was started in: `HostEnvironment::from_process()` is
production's own read of it, and the v0.1 workspace's Git children inherit
it. There is no seam to filter either at, so the environment is prepared
instead —
[`without_ambient_replacement_controls`](../../../src/workspace_manager/fixture.rs)
states what it takes away and why, and
`assert_replacement_controls_pinned` refuses in the child if any of it
survived.

The spawn itself was this module's own function until round 4. It is
`fixture::run_replacement_witness_child` now, shared with
`src/workspace_manager/tests.rs` and `src/gates.rs`, because three copies of
a door is three places a later repair can reach two of.

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

## `fn a_checkout_a_replacement_rewrote_is_refused_with_a_check_the_reader_can_run() {`

Both refusals of a checkout that differs from what HEAD records while
`git status` finds nothing to differ. `replace_head_with_a_sibling` replaces
HEAD with a sibling commit whose `README.md` differs and checks HEAD out through
the replacement, so the file on disk and the index are the sibling's. Plain
`git status` reads the same replacement and reports the checkout clean; upstroke
reads the recorded commit and finds `README.md` modified. Until round 3 of #326
the run's refusal said only "working tree is not clean; commit or stash first",
which `git status` contradicts, and there `git commit` finds nothing to commit
and `git stash` no local changes to save (git 2.43.0, measured). The fixture
asserts both views before either refusal runs, and each refusal is asserted to
name the command that shows upstroke's view,
`git --no-replace-objects -c core.useReplaceRefs=false status`, and the one that
lists replacements, `git replace -l`.

**Both controls, and the fixture pins `core.useReplaceRefs = true` to show why.**
Round 3 named `git --no-replace-objects status`. On Git 2.40.0 and 2.41.0, with
`true` in the repository's configuration, that command printed nothing and exited
0 over this checkout, while the command the refusals now name printed
`M  README.md` there and on 2.42.0 and 2.43.0 (measured with plain Git; the
review of `6e3e618f` found it). `replace_head_with_a_sibling` pins the value
before it installs the replacement and runs the command the refusals name, so the
fixture's own assertion is the check, on whichever Git the suite runs.

The first leg is the run's start. The second parks a run, switches back to
`main` and makes the same checkout there, so the refusal that fires is the
resume's, off the run branch. Reverting either message to its `915c0646`
wording turns the test red (`r3/` in the pull request's evidence).

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

## `fn resume_answering(repo: &Path, run_id: &str, effect: Effect) -> RunReport {`

Resume and answer the question the parked task is waiting on, so the
task actually runs again and its gates actually execute.

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

## `fn rewrite_run_started_as_schema_two(paths: &RunPaths) {`

Rewrite a current start into the shape written immediately before the
complete-review contract: schema 2 and no per-pass timeout field.

## `fn truncate_log_before(paths: &RunPaths, event: &str) {`

Rewind a log to just before the named event — the shape a process
killed at that instant leaves behind.

## `fn truncate_log_after(paths: &RunPaths, event: &str) {`

Rewind a log through the named event — the shape a process killed
immediately after its durable transition leaves behind.

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
open — and `upstroke answer` will accept a second answer against it,
one no engine can ever ingest, because the log has already closed
the question.

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

## `fn program_stem(program: &str) -> String {`

The file stem of a program path, however it was spelled.

## `#[test]`

`decisions.pr_sequence[5].scope`: "probes, workers, gates, reviews go
through the Runner", and `invariants_preserved`: "legacy engine behavior
unchanged (**the legacy engine does not run the shell probe**)".

One run of two tasks, each with two gates and one review pass, driven
through a recorder wrapped around the real host runner. What it establishes,
in the order the contract asks for it:

1. **Every** process the run started is a Runner request — the count is the
   run's shape (2 workers + 4 gates + 2 reviews), so a process that had
   gone round the seam would leave the count short and a process that had
   been added would leave it long.
2. The identities are the packet's first form in the **legacy generation**,
   written out by hand, and they are unique.
3. **No `probe(shell)` request**, and the recorder can see one — the same
   recorder is handed a real shell probe afterwards and the count moves
   from 0 to 1, so the zero is a measurement rather than a blind spot.
4. **Authoritative Git never crosses the boundary** (DESIGN.md:612): the
   run made commits, and not one recorded request is a `git` process.

## `let expected_ids = vec![`

(1) and (2): the exact identities of the run, in order, written from the
plan's shape and the packet's grammar rather than read back from the
engine. Two tasks at plan positions 0 and 1, one attempt each, generation
0 because the legacy engine has none, and inside each attempt the worker,
then gate 0 and gate 1, then review pass 0.

## `use crate::runner::ExecutionRole;`

The roles, and what each buys. R3, via `ExecutionRole::is_slotted`: a
worker and a review take an {agent, pool} pair; a gate does not, because
a gate is repository-controlled code and runs no agent CLI — which is
also why it is handed no agent.

## `let worker_stdin = &seen`

The prompt still reaches the child the way the adapter says it should.
`stdin_payload` is delivery policy and the spec is what carries those
bytes, so the two routed agent processes must be carrying them: a worker
gets the materialized task prompt, a reviewer gets the verdict prompt,
and a gate — which is a shell command, not an agent — gets nothing.

## `let shell_probes = |seen: &[RoutedProcess]| {`

(3) The clause, and the control that makes it a measurement. The run has
a recorded shell and ran four gate commands through it, and still never
asked that shell to `exit 0` through the Runner — `gates::shell_available`
is a PATH check and stays one.

## `assert!(`

(4) Authoritative Git never crosses the boundary. The run committed both
tasks — every one of those commits was git work — and not one process
the runner was asked to execute is git.

## `let worker = seen`

The workspace is the runner's to set, and it set a different one for the
worker than for the gates and the review: an attempt edits the repo,
while gates and reviewers judge the frozen candidate snapshot.

`same_path`, not `==`: the runner is given the workspace root the run
resolved and this test holds the `temp_dir()` name it created the repo
under, and those are two spellings of one directory on any host whose
temp directory is reached through a symlink (macOS: `/var` →
`/private/var`) or whose user directory has an 8.3 short name (Windows
CI: `RUNNER~1` for `runneradmin`). Comparing the directories rather than
the strings is also what makes the `!` case below mean anything — an
inequality between two spellings holds for free.

## `#[test]`

Every identity a *retried* attempt with two review passes and a re-ask
assigns, recorded from production rather than constructed by the test.

`the_legacy_engine_routes_every_process_through_the_runner` above records a
run whose every task runs once, with one review pass and no re-ask — so the
three fields that vary *inside* an attempt's identity never vary in it:
`AttemptNumber`, the review pass index, and pass-versus-re-ask. Each of
those is a distinct call site in `engine::attempt`, and a call site that
passes a constant where it should pass its argument is invisible to a grid
of hand-built identities (`runner::contract::tests::invocation_ids_are_unique_within_a_run…`
synthesizes its tuples; `review::tests::the_one_format_reask_is_its_own_invocation…`
is handed a correct pair). `invocation_identity` requires "unique per
process" and "a retry attempt has a new attempt number", and INV-20 makes
`review_pass(n)` and `review_reask(n)` distinct members.

So: one task that fails its first attempt and is retried, two gates, two
review passes from two families, and a first verdict the reviewer botches.
The expected list is written from the packet's grammar and this run's
shape.

## `vec![Effect::NoEdit, Effect::EditFile],`

The first attempt reports success and edits nothing, which fails
outcome sanity before any gate runs; the second does the work.

## `vec![ReviewBehavior::Unparseable, ReviewBehavior::Pass],`

The primary reviewer's first verdict is prose, so the pass spends
its one format-only re-ask and then answers.

## `"k0.g0.a1.worker.o0",`

Attempt 1: the worker alone — nothing downstream of a lying
agent runs.

## `"k0.g0.a2.worker.o0",`

Attempt 2 carries a *new attempt number* through every process
it starts, not only through the worker.

## `"k0.g0.a2.review_pass0.o0",`

Pass 0's verdict, and the one re-ask it is allowed — two
processes, two identities, and the second is not a second run of
the first.

## `"k0.g0.a2.review_pass1.o0",`

Pass 1 is the other family's, and it is pass *one*.

## `use std::collections::BTreeSet;`

Hostility as counts, so the list above cannot be satisfied by a run that
exercised fewer of the varying fields than it claims.

## `assert_eq!(`

`reviews_run` counts *invocations*, so the primary's two are the
verdict and its re-ask — the same two the identity list names.

## `#[test]`

A worker that cannot be spawned is an **infrastructure error**, not a task
failure — and the engine synthesizes no settlement for it.

`expected_failures_refusals[2]`: "a spawn failure uses the existing
runner/engine semantics: returned error; **no halting settlement is
synthesized**" (at integration, PR8's Deferred/Parked outcomes handle it).
Every fake worker in this file returns a spawnable shell command, and the
reviewer's spawn failures are converted to `ReviewUnavailable` *inside*
`run_review` before they ever reach the coordinator's error arm — so
nothing here exercised that arm, and converting it into `fail_task` would
have left the suite green while turning an outage into an attributed task
failure with a halt.

The oracle is threefold, because "returned error" alone would also be true
of a run that had recorded a failure first: the call returns `Err`, the
error is the runner's own spawn diagnostic, and the log carries
`attempt_started` with **no** settlement after it.

## `let run_id = rundir::latest_run(&repo).expect("the run created its directory");`

Nothing was settled: one attempt started, no attempt finished, no task
failed, and the ladder bought no second attempt.

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

## `#[test]`

**Every** public way to become a write coordinator establishes containment
first — not only the CLI's.

`invariants[INV-18]` is "on Windows every host child is a member of the
**coordinator's** ambient kill-on-close Job Object *from creation*", and
`expected_failures_refusals[1]` is "ambient job cannot be created or joined
(Windows) → write command refuses at startup with a diagnostic". Neither
says "when the coordinator was started by `src/main.rs`".
`decisions.phase_zero_modules.modules["src/engine/mod.rs"]` freezes
`run/run_with/run_harness` and `resume/resume_with/resume_harness` as the
facade, so all six are supported entry points: a downstream crate calling
`engine::run_with` is a write coordinator, and until this test existed it
established nothing at all. A kill between `CreateProcessW` and private-job
assignment then left the suspended stub alive — the exact residue INV-18
exists to prevent — and an ambient failure could not produce the required
pre-effect refusal, because nothing attempted the join.

The oracle is [`crate::runner::host::containment_establishments`], which
counts on the calling thread: "this call established containment", not "some
earlier call in this process did". Each entry point is driven with input it
must refuse once it is a write coordinator, so the assertion is about the
entry point and not about a run: an absent plan for a run, and for a resume a
recorded legacy run (`parked_run`) whose plan is gone, which the router passes
to the legacy lines, where containment is established before they refuse on
the plan.

An id the router refuses never makes its caller a write coordinator. Since
PR12-REG9-HUSK-REFUSAL-LEGACY-PUBLICATION the router returns
`rundir::resolve_run_id`'s refusal itself, read-only and before containment,
as it already returned its read and schema refusals (the documented resume
order: the router, then containment). So the three resume entry points are
also driven with an unknown run id, and each is refused with the resolver's
own refusal and no establishment. Until that repair the unknown id was this
test's resume input: it reached the legacy lines' containment only after their
prelude had published the include under the named root. The CLI is unaffected:
`src/main.rs` joins the ambient job for `resume` before the engine is called.

The *class* is what is asserted: the table is crossed against the facade's
own `pub fn` list, so this cannot be satisfied by covering the two the
review named.

## `let mut driven: Vec<&str> = entry_points.iter().map(|(name, _)| *name).collect();`

The class, not the instance: every public entry point of the facade is
in this table, and every row of the table is one of them.

## `#[cfg(windows)]`

Windows has the other half of the same fact: the coordinator process
really is a member of an ambient job now. (Process-wide and latching,
so it corroborates the count above rather than replacing it.)

## `#[test]`

The other side of the same census: a public entry point that is *not* a
write coordinator does not establish containment, and there are six of them.

`crash_reconstruction` anchors the ambient job at "every **write** command",
and `src/main.rs`'s `command_class` is the CLI's half of that split
(`every_write_command_establishes_containment_and_no_read_only_one_does`,
which asserts `skipped == 6`, "the six read-only subcommands"). This is the
library's half, and the six rows below are **the functions those six arms
call** — one per read-only subcommand, so the two censuses count the same
six things from opposite ends and the distinction survives when the CLI is
not involved at all.

`connect` and `capacity` are the interesting rows — they *do* spawn agent
CLIs — and they are still not coordinators, so INV-18's "the
**coordinator's** ambient … Job Object" does not reach their children.
Protecting those is a stronger guarantee than the packet asks for; it is
recorded as `PR4A-SPAWN-WITHOUT-AMBIENT` in `reviews/FINDINGS.md` with an
owner, not done here.

## `std::iter::empty(),`

No ids: the seam exists so this test spawns nothing. What
is under test is the containment step, which happens
before any spawn or not at all.

## `#[test]`

Containment comes **before** the coordinator, and a failure to establish it
refuses the run before any effect.

`side_effect_vs_event_ordering` is "no events; ambient job before any
spawn", and `expected_failures_refusals[1]` is a refusal "at startup with a
diagnostic". The oracle is `src/main.rs`'s: the two outcomes are different
errors from different places. A refused containment names the ambient job
and *not* the plan; with containment established the coordinator runs and
fails on the plan instead. If establishment happened after the coordinator,
or not at all, the first call would carry the plan's error.

The step is a parameter here for the reason `dispatch` takes one: no machine
can make the real join fail on demand, and on Unix it cannot fail at all.
The seam is not a hole — `Contained`'s field is private to
`crate::runner::host::proof`, a module with no descendants, so a closure that
returns one has established containment.

## `#[test]`

The same ordering, for the other coordinator. A resume is a write command:
`startup_census` enumerates them "(run, resume)".

## `let looked_in = repo.display().to_string();`

The resume's own refusal names the run directory it looked in; the
containment refusal cannot, because it happens before the coordinator
resolves anything.

## `const PARKING_SETTLEMENT_KILL_CHILD: &str = "engine::tests::parking_settlement_kill_child";`

The first kill child of the question-payload witnesses: the run dies once its parking settlement
is durable.

## `const QUESTION_PAYLOAD_KILL_CHILD: &str = "engine::tests::question_payload_kill_child";`

The second: the payload's write dies at one of its phases.

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
  says so with the length before Git's "Filename too long" does. On the CI guest
  the longest is 249 characters, for the shapes witness's glob directory.
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

## `fn the_v1_conductor_keeps_every_role_on_the_recorded_graph_of_its_own_repository() {`

The round-4 acceptance witness. Its child comes through
`run_challenged_replacement_witness_child`: every ambient control is taken away
and then the challenge is set, `true` in the system and global files, in
`GIT_CONFIG_COUNT` and in inherited `GIT_CONFIG_PARAMETERS`. The child adds
`true` in the repository and worktree files; every worker and reviewer is
started with `true` in its overlay; every gate sets it in its own snapshot.

The first run's worker, gate and reviewer each probe, and a scripted review
failure parks the task after all three have run. The include is then deleted and
the task resumed; the resume writes it back, and its worker, gate and reviewer
probe again. After each: every record line is `ok`, the operator's Git read the
replacement throughout, the configuration files are byte-identical, and the
include holds its exact bytes. The committed `candidate.txt` is the worker's
bytes, not the replacement it installed.

Under the v0.1 runner put back on the process-wide variable, it fails: the roles'
fixtures read `recorded`, the variable is present, and the include is not in
force. On Git 2.41.0 and 2.42.0 it passes (measured, with those Gits first on
`PATH`).

## `fn a_gate_that_replaces_objects_in_its_own_fixture_passes_under_the_v1_runner() {`

The regression review's own reproduction, in a clean environment: a managed
repository with nothing under `refs/replace/`, whose worker, gate and reviewer
each create a repository of their own, install a replacement and read it back.
The run completes. With the process-wide variable it parked with `gate_failed`.

## `fn a_refused_an_interrupted_and_a_resumed_v1_run_leave_git_configuration_as_they_found_it() {`

Under the same challenge as the acceptance witness: a run refused over a stray
file, a run killed inside its first attempt (`v1_interrupted_run_helper`, the
crash pattern `killing_a_run_mid_attempt_leaves_a_resumable_record` uses), and
the resume of the killed run after its include is deleted. The configuration
files are byte-identical after each; the resume's roles read the recorded graph.

## `fn sibling_v1_runs_in_one_repository_share_one_include_and_keep_their_roles_recorded() {`

Two runs at once, in two linked worktrees of one repository with replacements,
each in a process of its own, over one private root that holds no include yet.
Both complete; every role of both read the recorded graph; the shared include
holds its exact bytes afterwards; no configuration file changed. The helper
waits for both runs before judging either. A failing sibling's assertion used to
unwind the helper, and its scratch-tree guard deleted the tree the other sibling
was still running in. The sibling
checkouts are created with `--no-replace-objects -c core.useReplaceRefs=false`,
because a checkout written through the commit replacement differs from what its
HEAD records and the run refuses it as dirty, correctly.

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

## `fn a_gates_worktree_setting_is_attempted_again_only_past_another_registrations_empty_commondir() {`

The loop against scripted raw outputs: past two tears of another registration
to the setting. Returned at once, each after one attempt: the same line with
exit 1; the same line unfinished; the same line with an empty line after it; a
second line before it; two lines, each Git's death; the gate's own
registration, and Git's numbered spelling of it; a removal's; a lock's; an
unrelated refusal; and, on Unix, the same line from a Git a signal ended (a
synthetic status, signal 15). A deadline already past returns the first tear; a
success, and a command that cannot start, at once.

## `fn a_gates_worktree_setting_answers_what_the_probe_answers_when_nothing_tears() {`

Against Git itself, in a scratch repository and a directory that does not
exist: a success with output, an unknown revision, an unknown option (Git's
usage text, several lines, trimmed) and a `-C` that cannot be entered each get
the answer `probe_git` gives the same command, to the byte.

## `fn the_gate_role_sets_its_worktree_value_again_only_past_another_registrations_empty_commondir() {`

Unix only. The call site itself: the gate role (`v1_role_probe_gate`, this test
binary run as a child) in a directory named as a gate snapshot, with a spec
asking for `worktree true` and `PATH` leading to a stand-in `git` script, which
answers the setting once as the case says and then succeeds, records each
setting call, and fails every other command (so the role's other checks fail
and it exits 1, which the test does not read). The script reads where it records
its calls, and the marker of its one answer, from two environment values it
quotes (`UPSTROKE_W924_CALLS`, `UPSTROKE_W924_ANSWERED`), and `PATH` is joined
from the paths themselves, so a temporary directory whose path holds a quote, or
a byte that is not UTF-8, reaches the script as it is; one holding `:`, which no
`PATH` can name, fails on `join_paths`. (Until the B10 round those paths went
into the script as display strings inside single quotes, and `PATH` from the
stand-in directory's display string, so a quote in the temporary path broke the
script and failed the first case: B-I9-1, the follow-up B record's §9.27.) The
record's `worktree-true`
line: `ok` after two calls for another registration's tear; `FAIL` with the
first answer's standard error after one call for the same line with exit 1,
from a Git `SIGKILL` ended (no inherited disposition can ignore it), unfinished,
or with an empty line after it, and for the gate's own registration and an
unrelated failure. With the call site put back to `probe_git`, the first case
records `FAIL`.

## `fn the_v1_include_names_the_managed_repository_however_its_path_is_spelled() {`

One run per shape, each over a repository with the blob, tree and commit
replacements: a path with `[x]*?` (`[x]` on Windows), spaces and a quote in it; a
separate Git directory; a linked worktree as the entry; on Unix, a symbolic link
as the entry and a `.git` that is a symbolic link to the Git directory; on
Windows and macOS, the entry spelled in upper case. Every role of every run read
the recorded graph, and afterwards the operator's Git still reads the
replacements.

## `const TORN_REGISTRATION: &str = "foreign-torn";`

The tests appended by follow-up D (adopted with decision O8 on 2026-10-11 as an agent decision by
`pr11_decide_d` under the owner's delegation of 2026-10-11 (babysit's `0194a1e4…`), and in force
only from #331's merge;
`reviews/2026-10-02-pr11-follow-up-d-record.md`, its Implementation section) drive the
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

## `fn a_registry_refusal_after_capture_keeps_and_pins_the_captured_candidate_across_both_resumes() {`

T-P1, T-P3 and T-P5. A tear that stays, planted right after the capture: the run ends as
a registry refusal naming the kept pin, the checkout still holds the candidate, and the
pin's commit has the captured tree and parent. The resume while the residue stays
refuses at its reclaim and changes nothing: the status and the pin are as before. After
the repair the resume completes, names the pin, and leaves it. Red at master, where the
snapshot's Git error discards the output; red when the refused arm discards
(`m-discard`) and when the resume removes kept pins (`m-removepin`).

## `fn a_kept_pin_holds_the_captured_tree_when_the_index_changed_after_capture() {`

T-P2 (FUB-D5-INDEX). The index is unstaged after the capture; the pin holds the captured
tree, not the index's, and after the resume discarded the checkout's copy, following the
warning brings the worker's file back. Red when the coordinator pins the live index and
`HEAD` (`m-index`).

## `fn following_the_kept_pin_warning_at_its_parent_restores_deletions_too() {`

T-P4 (FUB-D5-RESTORE). A tracked edit, a new file and a deletion: following the warning's
restore leaves the checkout and the index exactly the pin's tree, deletion included. Red
under round 5's `git checkout <pin> -- .` (`m-restore`).

## `fn following_the_kept_pin_warning_on_a_later_head_applies_exactly_the_kept_change() {`

T-P4b. The resumed run commits the task again, so `HEAD` is past the pin's parent; the
warning's command for a later `HEAD` stages exactly `A new.txt`, `D deleted.txt` and
`M tracked.txt`, and the new commit stays.

## `fn a_topology_slots_torn_registration_refuses_a_legacy_snapshot_and_keeps_its_output() {`

T-P6, (e2′) itself: the residue is a topology slot's registration, which the workspace
manager adds (`a_torn_topology_slot`, through its funnels) and its fixture's
`tear_registration` tears, in the same repository's shared store. The run is refused
with the candidate kept and pinned; once the topology writer finishes, the resume
completes, names the pin and keeps it.

## `fn an_attempt_error_that_is_not_a_registry_refusal_keeps_the_output_pinned() {`

T-P7 and T-P7b (`a_snapshot_failure_that_is_not_the_registrys_keeps_the_output_pinned`),
reconciled by R-D1's part N (the proposal's §7, round 2's hunks): an attempt error that is not
a registry refusal — the after-capture step's own error, or a snapshot store that cannot be
made — keeps the checkout and pins it, and the error stays the attempt's own: T-P7's is the
`Git` error naming "not the registry's" inside `WithWarnings`, with the warning "is kept in
this checkout and pinned at"; T-P7b's is not `RegistryRefused`. Both were D's controls for "every
other error discards", which N reverses by design; no assertion about the error's identity is
dropped. Red when N1 discards again (`n1-discard`) and T-P7 when it pins nothing (`n1-nopin`);
T-P7b also when every snapshot error records the candidate (`m-keepall`).

## `fn a_kept_pin_that_cannot_be_written_is_reported_and_nothing_is_discarded() {`

T-P8 (R-D1), reconciled by part K: a ref below the kept pin's name (`…-kept/blocker`) makes
`prepare_commit_from_candidate` fail, where a moved `HEAD` no longer does: the refusal says
pinning failed, nothing is discarded, and no pin exists. Its FUB-D5-INDEX role on a moved `HEAD`
moved to T-R3 (`a_branch_moved_after_capture_keeps_the_output_pinned_at_its_captured_identity`).
Red under `m-discard` and when the refused arm discards on a failed pin (`m-pinfail-discards`).

## `fn a_review_snapshot_refused_after_capture_keeps_and_pins_the_candidate() {`

T-P9: the review snapshot's site — no gates, one reviewer — with the same result as T-P1.

## `struct OnceContended {`

The writer of a tear on its own thread, gated by the registry access's
`CONTENDED_ATTEMPTS` handshake for the repository's canonical common git dir
(`canonical_common_git_dir_of`), with a watchdog, as `src/workspace.rs`'s twin.

## `fn a_legacy_run_completes_past_a_tear_its_writer_finishes() {`

T-L6, (e1): the legacy engine completes past a tear its writer finishes after the
snapshot add's first failed attempt, with nothing discarded and no pin.
`a_legacy_run_completes_past_a_topology_slot_its_writer_finishes` is (e2) itself: the
writer is a topology slot's registration the manager added and tore, finished the way its
writer finishes it. Both red at master, where the first failed add is Git state and the
coordinator discards.

## `fn the_kept_pin_restore_writes_the_recorded_blob_under_a_blob_replacement() {`

T-P10 and T-P11 (FUD-D1-REPLACE, at the pin's parent): a blob replacement of the worker's
new file, and a tree replacement of the captured tree by its parent's. Following the
warning's restore leaves every path of the pin's recorded tree with that blob's bytes, no
other index path, and the index's tree the pin's. T-P12 and T-P12g
(`the_kept_pin_pick_applies_the_recorded_change_under_a_tree_replacement`,
`the_kept_pin_pick_takes_the_recorded_parent_under_a_graft`) do the same on a later
`HEAD`, under a tree replacement made at capture and a `git replace --graft` of the pin
made after the resumed run's commit. Red when the warning's commands lose their
replacement controls (`m-plain`).

## `fn every_kept_pin_is_named_after_a_second_refusal_on_the_next_resume() {`

T-P13 (FUD-D1-PINWARN, the concurrency lens's sequence): a refusal keeps attempt 1; after
the repair the resume's own attempt 2 is refused too (its worker plants the residue) and
kept; after the second repair the successful resume names both pins and removes neither.
Red when the lookup reads only the attempts still in flight (`m-interrupted`).

## `fn named_after_a_failed_resume(tag: &str, retire: bool) {`

T-P14 (`a_kept_pin_is_named_after_a_resume_that_failed`) and T-P15
(`a_removed_kept_pin_is_named_no_more`), the regression lens's sequence: a refusal, then a
resume whose worker cannot spawn, which settles attempt 1 as interrupted and fails before
its report (the replayed log shows attempt 1 no longer in flight), then a successful
resume. It names the pin, which survives; with `git update-ref -d <pin>` between the two,
it names none. T-P14 is red under `m-interrupted`.

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

## `struct ChildEngine {`

A resume or a run in a child of this test binary, its result written to a report file (`report_of`): the fixtures
`kept_engine_child_resumes`, `kept_engine_child_runs`, `kept_engine_child_runs_and_fails_after_capture` and
`kept_engine_child_runs_and_tears_after_capture`. A child is how a test gives the engine an environment of its own —
`GIT_TEST_SPLIT_INDEX` inherited, or a `git` shim first on `PATH` — with no `std::env::set_var` in this process.

## `struct GitShim {`

A `git` first on a child's `PATH` (Unix): it logs every invocation and runs each `ShimRule`'s action once, before the
matching invocation or after it with the real `git`'s status kept. A rule matches the subcommand, found past `-C <dir>`,
`-c <setting>` and other leading options, and a word the arguments hold. An action's own Git runs through `foreign`,
the real `git` with none of the engine child's own variables (`GIT_INDEX_FILE`, `GIT_NO_REPLACE_OBJECTS`), as another
client's would; without it a foreign `reset` meant for the checkout's index ran against the capture's private one.
`kill -KILL $PPID` in an action kills the engine at that exact child.

## `const EVIDENCE_HEAD: u16 = 2 * 1024;`

What the shim-fired assertions of T-N1c (`a_branch_moved_during_the_capture_keeps_the_output_pinned`) and T-RD2-2b
(`a_file_written_after_the_capture_is_never_written_over`) report when they fail. `assert_the_shim_fired` keeps each
assertion's condition, `shim.fired(0)`, and its words, and appends `unfired_evidence`, which `assert!` evaluates only
once the condition is false: a passing run reads nothing more. `#[track_caller]` keeps the panic's location at the
test's own call. The message is built before the panic unwinds; `ScratchTree` then attempts to reclaim the fixture tree,
and neither its `Drop` nor the absence of a warning proves that the files are gone. The retained records of the two
historical failures do not contain these files. The reclaim itself is unchanged.

Each line is a later observation, made after `exists()` answered false, and none replaces that answer: the marker and
the shim's directory, by kind alone; the child's report as `ChildEngine::run` returned it; then `calls.log`, the child's
log, the generated script and the test's own paths (`.git/HEAD`; `deleted.txt` and the die-inside child's log). Only
paths inside the fixture's tree, the shim directory's parent, are looked at, by their exact names, and nothing is
listed. The tree is opened by its path with `O_DIRECTORY | O_NOFOLLOW` (`evidence_parent`); every component below it is
opened by `openat` on the descriptor of the one above, with `O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC` (`open_at`), so a link
anywhere below the tree is refused rather than followed (the kernel answers `ELOOP` for a link as the last component,
and on Linux `ENOTDIR` for a link where a directory is opened) and a FIFO is opened without waiting for a writer. Only
what `fstat` on the opened descriptor calls a regular file is read (`evidence_at`); a FIFO, socket, device or directory
is closed unread. Each descriptor is an `OwnedFd` inside a `File`, closed when it goes out of scope; the two `unsafe`
blocks are the `openat` call and the adoption of the descriptor it returns, which nothing else owns, and each carries
its `SAFETY:` comment at its site, which §11 places there and these notes do not replace. `open_at` takes no flags from
its callers, only whether the component must be a directory, so the call cannot carry `O_CREAT` or `O_TMPFILE`, the
flags for which `openat` reads a mode argument. A file of at most 8 KiB is read whole; a larger one only in its first 2
KiB and its last 6 KiB, each into a buffer allocated once at that size. Each range is reported as requested and with
the bytes actually read, so a short read shows as one. The bytes are
shown by `<[u8]>::escape_ascii`, which is lossless; nothing is redacted; the whole text, the line that says it stops
included, is at most 64 KiB: what does not fit is cut at the last character boundary that leaves that line its room, so
the cut can fall in a line that did fit. The window sizes are `u16` so that both `u64` and `usize` take them by `From`.

What is missing, denied, refused, changed or truncated is reported as that, with the error's kind and text, and is not
read as a cause. The size and the modification and change times are compared across the read, and a difference is
reported. After the read the path is looked up again by name: a lookup that finds another file than the one read is
reported as that, and a lookup that fails is reported with its error's kind and text, an observation of that error and
not of the path naming no file. Not closed: the tree's own path is resolved as the operating system resolves it (on
macOS, `/var` is a link above every temporary directory); the observations are sequential, so the bytes read are those
of the file the held descriptors named at the open, the later lookup sees the path's binding only at its own moment, and
a change between the observations, or one undone before the later lookup, can escape them both; equal times do not prove
a file unchanged; a device node inside the tree would be opened, though none can be made there without privilege; an I/O
failure becomes text and a formatting failure a marked line, but an allocation failure aborts the process; and what the
files hold was written by processes that inherited the test's environment, which the evidence does not inspect.

## `fn the_unfired_evidence_reports_missing_and_unreadable_files_as_such() {`

The evidence's controls, each on a private scratch tree with its expected text written out: missing files, and an
unreadable one (mode `000`, after checking that the mode bit binds); a 1 MiB `calls.log` shown in its two windows, each
window's buffer of exactly its size, and a text past the cap cut, its closing line included, to within a character of
64 KiB, at a character boundary, and into the text before the line that did not fit when it leaves less room than the
closing line needs; a link as `calls.log`, a FIFO as the child's log (made by `mkfifo`), a directory as the script, a
link and a file on the way, and a path outside the tree, none of whose bytes appear.
`the_unfired_evidence_refuses_what_is_swapped_in_after_its_check` drives the open boundary
itself: after the descent and a check that sees a regular file, the file is replaced by a link to a file outside the
tree, then by a FIFO, and a directory on the way is moved aside and replaced by a link to one outside; `evidence_at`
refuses the link, opens the FIFO without waiting, and reads the file through the directory it already holds, its later
lookup reporting another file; and with a directory on the way made unsearchable after the descent (after checking that
the mode bit binds), the read through the held descriptor succeeds and the later lookup reports its `PermissionDenied`,
not an absent file. An open by path after the check would fail each: it would follow the link, wait on the FIFO, and
read through the substituted directory. Each FIFO case runs on a worker that `fifo_bounded` owns from its spawn to its
join, under the protocol written beside it, which §10 places at its site and these notes do not replace: a worker its
timed polls have not observed finished within 60 s is released by opening the FIFO for reading and writing, which gives
an open waiting on it its writer, and once a second poll observes it finished it is joined, and the case fails with the
worker's id, the releasing descriptor and the worker's late value; a worker's panic is resumed in the test; and if the
second 60 s poll expires the process aborts rather than leave the worker running unobserved. So a regression that waits
on the FIFO is expected to fail rather than hang. The polls bound only their own waits: the join after an observed
finish and the release open have no deadline of their own (the open is expected not to wait on a FIFO, as fifo(7)
documents for Linux and XNU's source reads for macOS, which no native macOS run has shown yet), and no test executes
the abort.
`a_fifo_control_releases_and_joins_a_waiting_worker_and_resumes_a_panicking_one` drives that protocol itself: a
worker that opens a FIFO by path, which waits for a writer, is reported after 100 ms as waiting, under the id it sends
the test, and has returned before that report reaches the test; and a worker's panic reaches the test as its own
payload. `the_shim_fired_assertion_fails_as_before_and_its_message_keeps_the_evidence` runs
`assert_the_shim_fired` under `catch_unwind` while the marker is absent: it fails as the two tests' assertions would,
and its panic message, the text the harness prints for a failed test, holds the test's words and then the evidence; the
same call passes once the marker exists. No ignored test is added.

## `struct PartWay {`

What stops a first resume's discard part-way: on Unix the leftover's directory read-only, after checking the mode bit
binds; on Windows the file held open by this process with `FILE_SHARE_READ` only, so that Git reads it and its unlink
fails. The proposal names `share_mode(0)`, which would deny Git's own read too: the capture would then not hold the file,
and the part-way stop it predicts would not arise.

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

## `fn a_foreign_index_reset_during_the_resumes_capture_cannot_change_the_kept_tree() {`

The tests of round 2's cases: T-RD1-1 (another client's `reset` before the capture's `write-tree`, Unix), T-RD1-2
(assume-unchanged), T-RD1-3 (the pin deleted after the copy's `list-heads`, and before the revert's `read-tree -m`;
the copy restores it after every reflog expired and `gc --prune=now`), T-RD1-4 (V1, V3), T-RD1-5 (an interrupted UTF-8
write), T-GRESET (the unlink and the write forms, healed for the second resume), T-N1a, T-N1c (a shim moves the branch
during the capture's diff; Unix), `an_error_after_the_worker_wrote_and_before_its_capture_keeps_the_output_pinned` (N1d),
`an_earlier_attempts_retained_output_is_kept_when_the_next_worker_cannot_start` (N1e: a same-session retry whose worker
cannot spawn), T-N2a to T-N2c (the settlement's line torn by `FailTheLegacyAppendNumbered<4>`, whose number the test
checks against the log), T-X10, T-X16, T-X18, T-X19 and T-X20, and the controls X4
(`every_kind_of_change_is_pinned_and_an_ignored_file_survives`), X6
(`a_resume_that_dies_before_its_revert_is_finished_by_the_next`) and X17
(`a_leftover_whose_name_is_not_utf8_is_pinned_then_reverted`, Linux: a name APFS refuses). T-X10's resumed attempt then
fails on what the discard left, so its leftovers warning is not delivered (R-D5): the test reads the log's
`run_resumed` record instead. T-X20's first resume runs with the event log read-only, so it stops after its guarded
discard having recorded nothing, and the second finds the same attempt in flight, as the design's two resumes in a row
do; the second does not refuse, and whether its attempt commits is the nested repository's to decide.

## `fn a_retry_after_a_deleted_pin_restores_it_from_the_untouched_copy() {`

The tests of round 3's cases: T-RD2-1, -1b (a shim deletes the restored pin before the revert), -1m (the pin moved to
another upstroke-identity commit), -1c (a `.partial` cut short, planted), T-RD2-2, -2r, -2p, -2u, -2n, -2b (a shim
writes the deleted file again before the revert), and T-RD2-3, -3e, -3s, -3w, -3h and -3p (each condition turns
`core.autocrlf` on through `autocrlf_through`, written as Git reads it; the first of two resumes dies inside its
attempt, so both passed the guarded discard).

## `fn a_capture_leaves_a_split_index_readable_and_pins() {`

The tests of round 4's cases: T-RD3-1 (N1's arm on a split index with `splitIndex.sharedIndexExpire=now`; the hook
records the `sharedindex.*` set at the arm, so the comparison sees only the pin's own writes), T-RD3-1g, T-RD3-1e (the
split forced by `GIT_TEST_SPLIT_INDEX` inherited by a child, for the run and for the resume), T-RD3-1f (Unix: a logging
fsmonitor hook; only its calls in the checkout's own directory are counted, since a snapshot's worktree is another
directory), `a_hostile_repository_keeps_its_index_readable_and_its_monitor_unqueried` (the proposal's §10.4 shared-state
check: split index, untracked cache and the hook together, the hook counted only while the engine runs), T-RD3-2 and
T-RD3-2r (a shim lists the run's public directory at the first `read-tree -m`: a fresh copy, renamed into place, beside a
planted one), T-RD3-2p (the run's public directory read-only: the resume cannot write its own copy, refuses, and
discards nothing) and T-RD2-1cr (a shim cuts the copy `bundle create` wrote and kills the resume, so the crash leaves
what a crash leaves: a `.partial` under round 4, a final name under `copy-in-place`). The fsyncs themselves are traced
outside the suite (record §5.17).

## `fn filtered_output(root: &Path) -> std::io::Result<()> {`

The tests of F1 of the owner's full review of #331 (`FUD-FULL-FILTERPIN`, repaired under the owner's instruction "we fix
F1 and record the rest"): a worker that writes a `.gitattributes` naming a clean/smudge filter (`*.dat filter=lfs`, the
trigger N1's own finding names), the filtered `paid.dat`, whose bytes are not text, and `agent-output.txt`. Every capture
refuses the filtered path before `git add` could run the filter, so N1's arm keeps the output unpinned.
`a_filtered_output_the_pin_cannot_capture_is_kept_and_its_warning_says_how_to_resume`: the error stays the capture's
filter refusal; the kept note says the pin failed, that nothing outside the checkout keeps the output, and what to do; no
pin and no copy exist; the checkout holds the three files byte for byte.
`a_resume_over_a_filtered_output_refuses_saying_how_and_pins_it_once_the_attribute_is_gone`: the resume refuses as a
resume refusal, not the bare Git error, naming the attempt, the filter, that nothing was discarded and what to do, and
pins and copies nothing; the advice is then followed (the attribute file copied out and removed), the next resume
completes, its kept pin holds the paid bytes, and its copy puts the pin back after `update-ref -d`.
`no_engine_git_child_runs_the_filter_a_worker_attributes_its_output_to` (Unix, as the suite's other configured-command
witnesses): a filter whose clean and smudge commands append to a marker, under a driver name of its own so that a
filter configured outside the repository, such as Git LFS's, cannot stand in for it; `git hash-object` shows that Git
runs it once it evaluates the attribute (the positive control), and no Git child of the run, of the refused resume, or
of the resume after the filtered path was moved out runs it. The first two are red at the head the review read; the
third is a control, green there too, and red when the pin's capture evaluates the attribute.

## `const MARKED_ATTRIBUTE: &[u8] = b"*.dat filter=paid-marker\n";`

The test of DF1R-1 (the regression review of `cc8ea2ed`, P3, executed there; corrected under the owner's instruction "we
fix F1 and record the rest"): F1's advice first offered moving the filtered path out of the checkout, which does not
clear a tracked path. `a_tracked_filtered_path_moved_out_stays_refused_and_is_pinned_once_its_attribute_is_gone`
commits `paid.dat` before the run; its worker rewrites it and writes a `.gitattributes` naming a filter. The run keeps
the output in the checkout with no pin and no copy, and the resume refuses. The operator then moves `paid.dat` out: it
stays in the index, so the next resume refuses with the same filter refusal, pins and copies nothing, and leaves the
run's event log as the run left it; outside the checkout, only the operator's moved file keeps the paid bytes. With the
path put back and the attribute file copied out and removed, as the advice says, the resume completes: its kept pin
holds the paid bytes and the rest of the output but not the attribute file, which only the operator's copy keeps, and
its copy puts the pin back. On Unix the filter's driver appends to a marker, with `git hash-object` as the positive
control (`a_marking_filter`), and no Git child of the run or of any resume runs it; elsewhere no driver is configured,
the refusal is the same and the marker is not checked. The words are checked last: the kept note, the refusal and the
refusal after the move each name the attribute's removal after copying out anything to keep, and none offers to move
the path out.

## `fn a_legacy_run_under_an_inherited_selection_checks_its_review_input_at_the_selected_repository() {`

The first of the three tests appended under option (C) (adopted by the owner on 2026-10-15
with O8's extension for it, the owner's adoption record `818e7675…`; this file's row in
`effects/allowlist.toml`, AMENDED TWICE). D1's layout: `A` tracks `work/shared.txt` and its user has staged another
version; `B.git`, selected by `GIT_DIR` and `GIT_WORK_TREE=A/work`, holds the
same file. An isolated child whose environment alone carries those two names
seeds a worker-only plan with review disabled and runs `run_with`, the real
coordinator and the real legacy ladder: the run completes, its task commits,
`B.git`'s run branch holds the worker's edit, and `A`'s index and `HEAD` are
byte-identical. Green at `5c222ff2` and under the change, because the ladder
asks the review-input seam's `problem` about its own root, which
`Workspace::open` opens as the started checkout; red when `problem` opens its
path as a work tree the engine made, because without the two names Git at
`A/work` finds `A`, where the captured tree is not.

## `fn inherited_selection_run_helper() {`

The child of the test above.

## `fn a_gated_legacy_run_under_an_absolute_common_dir_keeps_its_snapshot_in_the_run_repository() {`

C4's layout through the real coordinator: the repository's `.git/worktrees`
is a link into `other`, a copy of the repository, and an isolated child whose
environment alone carries the repository's absolute `.git` as
`GIT_COMMON_DIR` runs a gated `run_with`. The run completes with its gate
reported, the run branch holds the fake worker's `agent-output.txt`, which is
still in the checkout, and `other`'s objects and refs are byte-identical. The
gate snapshot is registered through the link; its Git keeps the absolute value
and reads the run's common directory. Removing `GIT_COMMON_DIR` in a work tree
sends its verification to `other`, "bad object HEAD", and the attempt fails:
before #331 the coordinator's discard then erased the worker's output, which
since #331 it keeps and pins instead (reasoned for this layout until the
mutation runs on the integrated head). The seed's commit runs with the
repository's `maintenance.auto` off, so it starts no automatic maintenance,
which Git 2.47 and later detach, in the repository the layout copies.

## `fn absolute_common_dir_run_helper() {`

The child of the test above; its body is `gated_run_under_an_inherited_common_dir`.

## `fn a_gated_legacy_run_under_a_relative_common_dir_through_a_linked_registry_keeps_its_snapshot_in_the_run_repository()`

C5's production layout through the real coordinator: the repository six levels
below the scratch root, its Git directory copied to `common` at the root,
`common/worktrees` a link into `other`, and the run's private root one level
below the root, so its gate snapshots are six levels below it too. The child
carries `GIT_COMMON_DIR=../../../../../../common`, which names `common` from the
repository and from the snapshot, and the same assertions hold as for the
absolute case. Shedding a relative value in the snapshot instead of giving it
the path it names at the checkout sends the verification through the link to
`other`. The repository's `maintenance.auto` is off from `git init`, so no
commit starts automatic maintenance in the Git directory copied next, and the
copy names any file it cannot copy, the type it was listed as and what is there
now.

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
run ids. CI selects it by its exact name, after the full suite, in the `test` (Linux, macOS) and
`test-windows` (guest) jobs:

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

## `fn a_schema_4_resume_proves_its_owner_and_commit_records_before_it_writes_its_include() {`

PR12-CI-01 (P1, concurrency/invariants lens) and
PR12-RESUME-INCLUDE-BEFORE-RECORD-VERIFICATION (P2, regression lens): one defect, one
guard (INV-15, O18: the private root is derived read-only before any lock, and the owner
and commit records are verified before any private write). A real schema-4 run parks
through `run_with`; every resume below goes through `engine::resume_with` — the public
composition, which the recovery-order tests never reach — with no `--private-root`, so
the run's committed locator is the only root authority, and with the private half
present.

The premise holds that locator to the root the run was given, spelled as the code that
records it spells it: `run_started(4).private_dir` is the configured root through
`fs::canonicalize` (`prelock::authorized_private_root`) joined with `runs/<run-id>`
(`PreLockChecked::private_dir`), so the expected path is `fs::canonicalize` of the same
`private_root_for` joined with the same two components, compared by exact `PathBuf`
equality. The configured spelling is another spelling of that directory wherever the
temporary directory is not canonical — Windows' verbatim `\\?\` prefix, macOS's `/var`
for `/private/var` — and native CI run 41936043911 failed this premise on both before
any refusal ran, when it compared the locator with that spelling
(PR12-NATIVE11-WINDOWS-PRIVATE-PATH-ORACLE). On a canonical temporary directory, as on
the Linux runners, the two spellings are the same path.

Ten damages to the run's own records — owner record missing, not JSON, or naming
another run, repository, run directory, incarnation or runner (each owner field
`RecordsVerified::verify` compares); commit record missing, not JSON, or naming
another first line — each under both include states, refuse with the record's own
refusal. The locator retargeted, in the committed first line, to another existing root
of the conforming `<root>/runs/<run-id>` shape refuses too, both when that root holds
no records and when it holds copies of the run's own, which only the commit record's
digest of the first line tells apart, again under both include states. For every one of
those twenty-four refusals the whole persistent private root — and the foreign root for the
retargeted cases — is unchanged entry by entry, byte by byte and mode by mode, the log
is unchanged, every ref is where it was, and no probe ran.

The positive control resumes the provable run with the include absent and then changed:
the include holds its fixed bytes again, its directory is `0700` on Unix, the resume
reached its probes and appended `run_resumed`. With `resume_harness` publishing the
include before `ResumePrepared::verify` (the order before PR12-CI-01), the first refusal
fails on the private root that moved.

## `fn on_this_path(program: &str) -> bool {`

Whether a file named `program` is in a directory of this process's `PATH`; the
recorded-shell guards assert that `cmd` is not, so that "unavailable" is a fact of the
host and not an assumption.

## `fn a_fake_cmd(tree: &ScratchTree) -> PathBuf {`

A directory holding a `cmd` that exits 0, written by a process of its own
(`runner::container::write_program_in_its_own_process`) so that no thread of this test
binary holds it open for writing when it is executed.

## `fn with_shells_on_path(`

Runs [`recorded_shell_helper`] in a process of its own whose `PATH` starts with the
fake shell's directory, so the shell is available to that process alone and to no other
test.

[`RecordedShellScope`] owns that process and its process group. Its stdout and stderr
go to files in a scratch tree, not to pipes, so nothing it leaves running can hold the
wait open, and its stdin is null, as `Command::output` made it. The wait is bounded at
300 seconds, the bound
`a_legacy_run_under_an_inherited_selection_checks_its_review_input_at_the_selected_repository`
gives a whole run in a child of its own. Each caller requires a successful exit status
and `1 passed`, and prints the child's status, stdout and stderr when that does not
hold, so a child killed at the bound (status `None`), one that failed and one that
matched no test all fail with their diagnostics. The `Command::output` this replaced
waited without a bound for the child and for every holder of its pipes
(PR12-R7-RECORDED-SHELL-HELPER-UNBOUNDED, with its correlated
PR12-CI2-RECORDED-SHELL-HELPER-UNBOUNDED).

## `struct RecordedShellScope {`

The recorded-shell helper's process group, owned on every way out. An anchor
(`/bin/sleep`) starts first, leading a group of its own, and the helper joins that
group. The anchor is collected only after the group has been killed, so the group id is
this test's for the whole scope — after the helper has been collected, and even if the
anchor has exited — and the kill cannot reach a process that reused it. `settle` sends
`SIGKILL` to the group and then collects the helper and the anchor within
`ISOLATED_CHILD_COLLECTION_BOUND`. It runs when the helper ends, successfully or not,
so whatever of its descendants stayed in its group dies with the anchor; at the bound;
when polling the helper fails; and from `Drop`, so a panic between the helper's start
and its settlement settles it too. A settlement that cannot collect both within its
bound fails the test or, while unwinding, says so, and leaves the rest to this process's
exit. Descendants that left the group, as the engine's commands and their reapers each
do, are the engine's to settle, as after any run. `agent::proc::test_support::run_test_isolated`,
which this fix first used, kills its child's group only at its deadline: it collects a
child that ended by itself without settling the group, and a failed poll panics with the
child unkilled and no guard to settle it.

## `fn recorded_shell_helper() {`

The recorded-shell guards' child. `create` makes and parks a zero-gate run under the
config it is given; `resume` resumes the latest run under the config it is given and
asserts that it appended `run_resumed`. Without its environment it does nothing.

## `fn a_zero_gate_schema_4_resume_probes_its_recorded_shell_when_todays_config_names_an_unavailable_one()`

PR12-CI-03 (INV-23), one direction of today's drift. A zero-gate run is created under
`sh` (the premises: `gate_cmds` is empty, `run_started(4).shell` is `sh`, and this host
has no `cmd`). Its resume through `resume_with`, with today's config naming
`shell = "cmd"`, passes its pre-flight, recovers (one `run_resumed`) and warns that
today's `cmd` is not the run's and that the pre-flight probes `sh`. With the resume's
shell taken from `gate_cmds` else today's config again, it fails: the pre-flight probes
`cmd` and refuses.

## `fn a_zero_gate_schema_4_resume_refuses_its_unavailable_recorded_shell_whatever_todays_config_names()`

PR12-CI-03 (INV-23), the other direction, and the unavailable recorded shell. A
zero-gate run is created under `cmd` by a child whose `PATH` holds a fake `cmd` (the
premises: no gates, `run_started(4).shell` is `cmd`, and this host has no `cmd`). Its
resume in this process, where `cmd` is not on `PATH`, with today's config naming the
available `sh`, refuses at the pre-flight naming the recorded shell `cmd`, and the log
is unchanged, so no recovery event was appended. The same resume in a child whose
`PATH` holds the fake `cmd` again continues and appends `run_resumed`: the run stayed
resumable. With the resume's shell taken from `gate_cmds` else today's config again,
the first resume probes `sh`, recovers and fails the refusal it expects.

## `fn a_schema_4_log_without_a_recorded_shell_is_refused_and_never_resumed_under_another() {`

PR12-CI-03, the record's compatibility (`run_started(4).shell` is required, as every
schema-4 transaction-payload field is). A parked run's committed first line, rewritten
without `shell`, stands for a log written before the field. Its resume through
`resume_with` refuses at the committed first line, naming `shell`, writes nothing to
the log and nothing under the private root, and never resumes under a shell the record
does not hold.

## `enum SignalFault {`

How [`signal_mid_attempt`] is made to fail, for the tests of its own cleanup. Each
fault is injected once the helper's worker runs. `Readiness` reports the readiness
the driver waited for as failed, and `Signal` the signal it sent, as a real failure
of either would be (J8, PR12 regression review #5). `DeliveryNeverExits` makes the
command that delivers the signal one that never exits: a stub standing where `kill`
stands, which records its pid and then sleeps for ten minutes (J15, regression
review 2, finding 4). `None` injects nothing.

## `fn stalled_delivery_pid(repo: &Path) -> PathBuf {`

Where the stub of `SignalFault::DeliveryNeverExits` records its pid, beside the
repository, published by a rename. The J15 test reads it after the driver has
failed, to find that process gone.

## `fn delivery_program(repo: &Path, fault: SignalFault) -> PathBuf {`

The program that delivers the signal: `kill`, found on `PATH` as before, or, for
`SignalFault::DeliveryNeverExits`, the stub, written beside the repository by a
process of its own (`runner::container::write_program_in_its_own_process`), so that
no other thread of this test binary holds it open for writing when it is executed.

## `fn deliver_within_its_bound(program: &Path, signal: &str, pid: &str, repo: &Path) -> bool {`

Delivers `signal` to `pid` by running `program -s <signal> <pid>` as a
[`LinkedChild`](crate::workspace_manager::fixture::LinkedChild)
(`LinkedChild::spawn_program`) and waiting for it with `wait_within(COLLECT_BOUND)`,
the bound the helper's own end has. A delivery still running at the bound fails the
driver, naming the command; as the failure unwinds, the link's drop kills and reaps
the command within its own bound, so it outlives neither the driver nor the test.
`true` when the command exited successfully.

## `fn signal_mid_attempt(`

The signal test's driver. The helper, this test binary running
`topology_signalled_child`, is owned from its spawn by a
[`LinkedChild`](crate::workspace_manager::fixture::LinkedChild), which kills and
reaps it within its bound however this function ends. Every wait the driver makes
is bounded: the readiness poll, the delivery of the signal
([`deliver_within_its_bound`]), the helper's end and its cleanup, its reader's
settlement, each worker group's end and the run's release. At the reviewed head of
J8 the helper was a bare `Child`: an assertion before its unbounded `wait` unwound
past it and left the helper and its worker running (J8, PR12 regression review #5).
`helper` learns the helper's pid as it is spawned.

J15 (regression review 2, finding 4): J8 left one wait unbounded, and said so. The
`kill` that delivers the signal was a `Command` whose `.status()` had no deadline:
a `kill` that never exited held the driver inside that call, before either the
delivery's assertion or `wait_within(COLLECT_BOUND)` could run, and the test and
the helper it owned stayed live indefinitely. §12 binds every wait, and a
disclosure does not discharge that. The delivery is now itself a link, waited for
within `COLLECT_BOUND` ([`deliver_within_its_bound`]); `COLLECT_BOUND` and every
assertion are kept, the J12 ones below included.

J12 (regular review 3, finding 1): after its order assertions, the driver
asserts that the resumed run's report says its total is a floor
(`RunReport::total_is_floor`), and that it keeps exactly one interrupted attempt,
whose cost is unknown and whose model is the one recorded by the
`attempt_started` with the same task key, generation and attempt as the
`attempt_interrupted` the resume appended. At the reviewed head the report held
only the attempts that completed, with no warning, and printed the run's known
spend, `$0.04`, as its whole total.

## `fn the_signal_tests_helper_is_settled_when_the_command_delivering_its_signal_never_exits() {`

J15 (regression review 2, finding 4), the reviewer's guard: the signal's delivery
is the stub of `SignalFault::DeliveryNeverExits`, a command that never exits. The
driver fails, within its bound, naming the delivery that had not exited. Afterwards
the stub, at the pid it recorded, is gone (its link's drop killed and reaped it);
the helper is reaped; each worker's process group is gone within `GROUP_END_BOUND`;
and nothing holds the run.

Against the driver as J8 left it, with the delivery's `.status()` unbounded, the
test does not end: measured once, it was still running past libtest's sixty-second
notice, and the deadline put on it from outside (a session of its own under
`timeout`, 120 seconds) killed it; nothing it started outlived the session, and
the scratch tree its unwinding would have reclaimed was removed by hand.

## `fn settled_attempts(events: &[serde_json::Value]) -> usize {`

How many attempts the log settles: `attempt_finished` for each one that did not
succeed, `candidate_prepared` for each one that did, and `attempt_interrupted`
for each one whose coordinator did not see it end. Since J12 the schema-4 report
keeps the last kind too, so the tests that compare the report's attempt rows
with this count stay a comparison with every settlement.

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

## `fn a_schema_4_question_is_announced_before_its_sibling_is_released() {`

J11 (regression review 2, finding 2): a question a schema-4 run raises is
announced through the configured notifier while an independent task is still
held, under both drivers and after a resume. Four cells: a fresh run and a
resumed one, each under the step loop (width 1, a source that cannot be shared)
and the coordinator (width 2). The config is `mode = "never"` and `notify =
["cli"]`. For a resumed cell the first run, in this process, stops at a $0.015
budget once `first` has spent, before `asks` or `waits` runs, and the child
resumes it with a $10 budget.

The run itself is a child of this test binary (`topology_announcing_child`)
because the CLI notifier writes to the process's stderr, which the child sends
to a file. The test reads that file until it holds the notifier's line for
`asks` (`question … — parking asks; the run continues`) and `waits-started`
exists. Only then does it release `waits`, which the run cannot finish without,
so the announcement is shown to arrive before the sibling is released and while
the run is still going. The child must then end successfully. Under the step
loop `asks` runs before `waits` by key order, so the announcement precedes the
sibling's start; under the coordinator the two run together.

At the reviewed head nothing was announced: with the announcement removed from
`RunEmitter::emit`, the first cell fails at its 90-second bound with an empty
stderr.

## `fn topology_announcing_child() {`

The child of the test above: the fresh run (`run_with`), or the resume through
the recorded locator with today's config and a raised budget (`resume_with`),
under the driver the parent names, with a 600-second attempt timeout so the
held sibling is never timed out. It asserts the run ends parked on `asks` alone,
with `first` and `waits` committed.

## `const UNSPELLABLE_HOME_REPO: &str = "UPSTROKE_UNSPELLABLE_HOME_REPO";`

The variable that hands the J14 child its repository.

## `struct TreeSnapshot {`

What `every_path_under` saw: `entries`, every entry it listed, as a path
relative to the root, sorted; and `unread`, the entries it listed and could not
read, a directory it could not open by its listed name or an entry whose type
could not be read because it was gone. The J14 tests compare the entries and
carry the unread ones in their messages (K7).

## `fn every_path_under(root: &Path) -> TreeSnapshot {`

Every file, directory and link under `root`, by the names the platform lists
and the types its listing reports (`DirEntry::file_type`, which reads the
directory entry's own type and looks nothing up by name where the platform
reports one); a link is listed and not followed. A directory is descended by
its listed name; one that cannot be opened by it, or an entry gone before its
type was read, is noted in `unread` rather than ending the walk. A root that
cannot be read is noted the same way, as the empty path.

Until K7 each listed entry was `lstat`ed by its listed path, with `.expect`.
On CI's macOS leg (run 41724856814, job 126229500908, macOS 26 arm64, APFS, git
2.56.0) the J14 test panicked inside this walk with `an entry's own metadata:
NotFound`: an entry the listing had just named could not be found by its path.
The job log does not say which entry, or which of the test's walks: the panic
names neither. Two mechanisms fit it, and the repair holds against both:
- a name the platform lists but cannot look up by the bytes it listed (the
  coordinator's reading of a home named `private-\xff` on APFS). The walk no
  longer looks a listed entry up to learn its type, notes a directory it cannot
  open, and the J14 test also walks the home by the path it holds, so whatever
  the run might create under it, the include included, is still compared;
- an entry that existed when the directory was listed and was gone a moment
  later: git 2.47 and later detach `git maintenance run --auto` after a commit,
  and it holds `.git/objects/maintenance.lock` while it decides, so a walk
  begun just after the fixture's commit could list that lock and miss it. The
  fixture's repository now turns automatic maintenance off before its commit
  (`temp_engine_repo`), and an entry gone before its type is read is noted
  rather than fatal.

## `fn a_schema_4_run_refuses_an_unspellable_default_private_root_before_it_creates_anything() {`

J14 (regular review 3, finding 3; regression review 2, finding 5): a fresh
schema-4 run with no explicit private root and a home no string spells is
refused before anything is created, the recorded-objects include included.
`HOME` is process-wide, so the run is a child of this test binary
(`unspellable_default_root_child`, through `run_test_isolated`, whose output
files live in a scratch tree of their own), with `HOME` and `USERPROFILE` set to
`<tree>/private-\xff`. Two cells: that home exists and is empty, and it does not
exist. A host that refuses the name (`EILSEQ`) cannot hold the first, which is
then skipped; the second needs no such name on disk. Every path under the
scratch tree, the repository and the home included, is the same after the
child as before it, and there is no `.upstroke` under the home. The comparison
comes first, so a regression shows what it left. With the read-only check
removed from `prepare_fresh`, the first cell fails on exactly the reviewers'
witness: `private-\xff/.upstroke`, its `git` directory and
`recorded-objects.gitconfig` appear, and the run is refused later, by
`authorize`.

Since K7 (CI's macOS leg) each snapshot is two walks: the scratch tree, by the
names the platform lists, and the home, by the path the test holds, which is
the path the child's `HOME` names. What is compared is every entry listed; the
entries a walk listed and could not read are carried in the message, before and
after, as the record of what the platform would not show. On a host that can
name the home, the home's own walk is the one that sees inside it however the
tree's listing spells it; on one that refuses the name, the first cell is
skipped as before.

## `fn the_tree_snapshot_lists_by_the_platforms_own_names_and_types_and_notes_what_it_cannot_read() {`

K7: the snapshot lists a directory, a file in it, a link to nothing and a link
to the directory, by their names, without following either link, and notes a
root it cannot read as the empty path rather than panicking. The macOS case
itself (a listed name or entry that cannot then be found) is not reproducible
on this box; this pins the walk's own rules.

## `fn unspellable_default_root_child() {`

The child of the test above: a fresh run through `run_with` with no private
root, so the root is the default one under the child's `HOME`. It asserts the
refusal is the default root's own, says the root is not valid UTF-8 and that
nothing was locked or created, and that no run directory exists.

## `fn the_default_root_check_judges_the_path_the_root_will_resolve_to() {`

`prelock::refuse_unspellable_default_root` judges the path the default root
will resolve to, not its spelling: a missing root under a home text spells and
an existing root text spells pass; a missing root under a missing home no
string spells is refused, saying nothing was locked or created; a link no
string spells that resolves to the home text spells passes (where the host can
hold the link's name). The checks create nothing: the tree holds the home and
the link only.

## `fn a_frontier_escalation_over_the_threshold_parks_for_approval_then_runs_it() {`

Since J16 it also asserts that the approval question names its task (`t1`)
and that its context opens with the task line, `` Task `t1` — One ``: the
question is built by `build_question` and described from the plan task it
names, and must read as it did when `build_spend_approval` indexed the plan
itself.

## `const INHERITED_COMMON_IMAGE: &str = "upstroke-tests/inherited-common:1";`

The image reference J10's guard configures and records. No runtime holds it,
and none is asked: the guard's stub runtime fails every command it is given.

## `const INHERITED_COMMON_REFUSALS: &str = "UPSTROKE_INHERITED_COMMON_REFUSALS";`

The variable that names, to J10's child, the file it writes both refusals to.

## `fn run_directory_names(repo: &Path) -> Vec<String> {`

The names under the repository's runs root, sorted: what J10's guard compares
before and after its children, so a refused run that left a run directory
fails it.

## `fn record_a_container_runner_mounting(repo: &Path, run_id: &str, mounts: &[(&Path, &str)]) {`

Makes the parked run's committed `run_started` record a container runner whose
mounts are `mounts`, each source writable at its target, with the canonical
spelling and the identity a resolution would record, in the order given. The
runner's JSON is replaced in the first
line, which must spell it exactly once, and every other byte of the log is
kept, so a second call replaces the first one's runner. It stands for the
record a container run would have written: the public
fresh path cannot create one here without a container runtime, and a resume
reads nothing of the run but `run_started`, its frozen plan and its private
half before it refuses. J10 records one mount at `/data`; K1 records the
targets a fresh run refuses, which no resolution would write.

## `fn a_mount_inside_an_inherited_common_directory_is_refused_by_run_and_resume_before_the_runtime_is_asked()`

J10 (regression review 2, finding 1). A parked schema-4 run, then the
repository's `.git` copied to `common`, a sibling of the checkout, as the
reviewer's layout places it. A configuration selects the container runner with
a writable mount of `common/refs`, and the parked run's record is made to name
the same mount. A stub `docker`, first on the children's `PATH`, appends what
it is asked to a file and exits 93, so no real runtime can be reached.

For `GIT_COMMON_DIR` set absolute and set relative (`../common`, from the
checkout), a child of this test binary whose environment alone carries it
(`inherited_common_dir_mount_child`, under `run_test_isolated`) runs a fresh
run and a resume and records both refusals; the parent requires the child's
success and libtest's `1 passed`, and that both recorded refusals name the
canonical common directory as the source's container. Afterwards the stub
was never asked anything, so no image was inspected, no runner was built and
no launch plan was made; the recorded log is byte-identical; and the runs
root holds what it held.

With the fix removed (`MountContext` bound from the checkout's `.git`, the
pre-J10 entry), the absolute cell fails: the fresh run accepts the mount, and
its first runtime question, the probe, reaches the stub ("the container runtime
refused `probe`"). Even then only the stub could be asked.

## `fn inherited_common_dir_mount_child() {`

The child of the test above. `run_with` over the container configuration is
refused with `[runner] the mount … is refused`, its source lying inside the
canonical common directory, authoritative Git; `resume_with` on the parked run
is refused as recording that mount, for the same reason, with nothing
published, locked or written. It then writes both refusals to the file its
parent names, the evidence that it ran.

## `const RECORDED_TARGET_REFUSAL: &str = "UPSTROKE_RECORDED_TARGET_REFUSAL";`

The variable that names, to K1's child, the file it writes its refusal to.

## `fn a_recorded_mount_whose_target_a_fresh_run_refuses_is_refused_by_the_resume_before_the_runtime_is_asked()`

K1 (regular review 4: recorded mounts bypass target validation), through the
public resume, `engine::resume_with`. A parked schema-4 run; a stub `docker`,
first on the children's `PATH`, appends what it is asked to a file and exits
93, as in J10's guard, so no real runtime can be reached. For each set of
`resolve::fixtures::REFUSED_RECORDED_TARGETS` (the container's root,
`/upstroke/settings`, `/cache/../upstroke/settings`, and `/cache` with
`/cache/inner`), the parked run's `run_started` is made to record a
container runner mounting a real directory at those targets
(`record_a_container_runner_mounting`), a complete record that a resume
reads back from the log. A child of this test binary
(`recorded_target_resume_child`, under `run_test_isolated`) resumes it and
records the refusal; the parent requires the child's success and libtest's
`1 passed`, a refusal that names the run, the mount, the target rule's words
and that nothing was published, locked or written, and a log byte-identical
to the one the child was given. Afterwards the stub was never asked
anything, so the image's environment was not read, no runner was built and
nothing was recovered; and the runs root holds what it held.

The public resume asks the runtime before recovery does: `prepare_resume`
reads the recorded image's environment. With
`resolve::refuse_recorded_targets` bypassed, the child's resume reaches
the stub ("the container runtime refused `inspect-image-by-id`") and the
test fails on the root cell (measured before the commit). Recovery's own
check, which a resume reaches only after that question, is the subject of
`engine::topology::recover::tests`' K1 guard.

## `fn recorded_target_resume_child() {`

The child of the test above: `resume_with` on the parked run is refused, and
the refusal is written to the file its parent names, the evidence that it
ran.

## `fn a_schema_4_follow_prints_one_line_per_event_and_no_raw_control_whatever_the_plan_or_the_record_holds()`

L1 (regression review 3, finding 1; DESIGN §18). A schema-4 run of one task
annotated `id=t1` followed by ESC `[2J` completes, and `status --follow`, through
the public `status::load` and `status::follow`, prints exactly one physical line
per committed event, with no control character but the line breaks, and the
`attempt_started` and `task_merged` lines name the task as `t1\u{1b}[2J`. Then
`Observed::describe` is given that run's `attempt_started` with controls in
three recorded fields, its timestamp, its agent and its model: the line holds no
control, and the agent and model appear with theirs made visible. With
`describe`'s `one_line` removed, the follow assertion fails on the raw ESC bytes.

## `fn a_fresh_schema_4_run_refuses_a_staged_an_unstaged_or_an_untracked_change_before_any_worker_or_log()`

L2 (regression review 3, finding 2; DESIGN §20). Through the production entry
(`run_with` with the topology active), three repositories with one kind of dirt
each: a staged edit of `README.md`, an unstaged edit of it, and an untracked
file. Each run is refused with "working tree is not clean", before any worker or
reviewer is built (`ObservedStub` records none) and before any run directory, and
so any run log, exists; the operator's change is untouched. The control: one
clean repository is admitted twice in a row, its workers running each time, so
the first run's public directory does not make the second run's tree dirty.
Measured before commit: with the schema-4 dispatch's refusal removed the staged
case runs to completion and the test fails; with the exclusion removed from
`refuse_dirty_tree` the second clean run is refused and the test fails.

## `fn stub_pool_file(tree: &ScratchTree) -> PathBuf {`

One pool, `claude-max`, for the topology stub's agent (`claude-code`), so a run's attempts and
review passes are attributed to it. The file is in the scratch tree, outside the checkout: a
schema-4 run refuses a dirty tree (L2), and an untracked pools file would be dirt.

## `fn cut_log_after(`

A coordinator that died just after appending an event leaves exactly the committed prefix that
ends with it, because the log only grows at its end. This cuts a finished run's log back to that
prefix after the first matching line. The run is released before the cut, so `status` reads it
as interrupted, not running.

## `fn upstroke_status(repo: &Path, run_id: &str) -> String {`

What `upstroke status <run>` prints, through the public `status::load` and `status::render`.

## `fn ledger_cells(status: &str, id: &str) -> Vec<String> {`

The cells of the ledger row `status` prints for one task, split on whitespace. A trail with
several words spreads over several cells, so callers count the amounts from the end.

## `fn a_schema_4_status_prints_the_ledger_run_reports_for_a_completed_and_an_interrupted_run() {`

L3 (regression review 3, finding 3; DESIGN §18). Two dependent tasks on one pool, completed.
- `status` prints, byte for byte, the ledger the run returned (`RunReport::render_ledger`): the
  task rows, the known total, the per-pool drain.
- Both fast integration rows end `(0 review(s), cost none)`.

The same log cut just after the second task's `attempt_started`:
- `status` reads the run as interrupted. The first task's row shows its exact known spend.
- The second task's row counts the cut-off attempt and knows no amount (`—`).
- The total and the pool drain carry `?`, and the floor legend is printed.

Measured before commit, each mutation failing this test:
- with the ledger removed from `Observed::render`, on the completed comparison;
- with the interrupted view's settlement removed, on the cut-off row (`t2 0 — — — —`, no floor);
- with `ran_no_review` removed, on the fast rows.

## `fn a_schema_4_status_prints_the_integration_review_spend_run_reports_and_its_floor_once_cut_off()`

L3, the integration reviews (Unix: the worker writes a file named by its own process). Two
independent tasks at width 2 make the second merge a stale candidate's re-verification, with one
priced review pass that the verified task pays for. Completed:
- `status` prints the ledger the run returned; the verified task's review cell includes that
  pass.
- The `stale_clean` row ends `(1 review(s), cost $X)` and the fast row `(0 review(s), cost none)`.

The log cut just after `merge_verification_started`:
- The verified task's review cell is its attempt's known pass with `?`, and the run's total is a
  floor.
- The integration row is `open` with `cost ? (unknown)`.

The ledger and settlement mutations above fail it too.

## `fn a_completed_schema_4_run_leaves_no_generated_file_for_an_ordinary_git_add_to_take() {`

L4 (regression review 3, finding 4). A real schema-4 run, through `run_with`, completes in a
repository that has no `.upstroke` exclusion before it (the premise is asserted). Its public
directory holds the run's log and report. Afterwards `git status` lists nothing, and an ordinary
`git add -A` stages nothing (`git diff --cached --name-only` is empty). With the exclusion removed
from `coordinator::refuse_dirty_tree`, it fails on `?? .upstroke/`.

## `fn a_schema_4_resume_restores_a_missing_run_exclusion() {`

L4, the resume. A schema-4 run parks on its one question, the answer is recorded, and the
exclusion its fresh run made is deleted. The premise is asserted: `git status` now lists
`.upstroke/`. The resume, through `resume_with`, completes the run. Afterwards the exclusion is
back and `git status` lists nothing. With the schema-4 branch's `ensure_run_exclusions` removed
from `resume_harness`, it fails on "the resume made the exclusion again".

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

## `fn assert_refused_under_a_held_lease<H>(`

The held-lease controls of PR12-R9-PUBLIC-PUBLICATION-BEFORE-WORKTREE-LOCK,
PR12-REG7-PRELOCK-WRITES and PR12-N4-PRELOCK-PUBLICATION (one defect), shared by the fresh and
resume guards below. For each of the four exclusion and include states (absent or changed, each)
it puts the states, takes the hold, snapshots the checkout outside `.git`, the private root and
every ref, runs the public command, snapshots again, and only then releases the hold: so whatever
taking the hold itself creates (a lease file, a reaper's open of `cleanup.lock`) is in both
snapshots and nothing is attributed to the command that it did not do. Each run must be refused
with the holder's own words, build no worker or reviewer, and leave every path, kind, mode and
byte in both roots, and every ref, as it found them.

## `fn assert_admitted_and_published(`

The positive control, run once the hold is released: with the exclusion and the include both
absent, the same public command is admitted, and afterwards the exclusion holds its fixed `*\n`,
the include its fixed bytes, and on Unix the include's directory is `0700`. It returns the report,
whose run is released before it returns.

## `fn a_surviving_reaper_holding(`

A surviving reaper's shared hold on one run's cleanup lease (R28), by the suite's own holder,
`rundir::tests::cleanup_hold_child`, through `workspace_manager::fixture::spawn_ready_helper`: a
child of this test, bounded by its 30-second `await_line` and by its own sleep, and killed and
reaped when the returned `Producer` drops. The premise — that the hold is observed — is asserted
before it is returned. Unix only, as the helper and the hold are.

## `fn a_fresh_schema_4_run_that_loses_the_worktree_lease_publishes_no_exclusion_and_no_include() {`

The fresh half of the one defect, through `run_with` with the topology active. A pristine
repository (the premise: no `.upstroke`) and its explicit private root; the run is refused under
the lease this test takes in-process (the conductor's case, "is already driving worktree") in all
four states, with nothing published in either root and no ref moved. Released, the same run is
admitted and completes, and makes the exclusion and the include from absent. On Unix, beside a
surviving reaper of that first run (R28's case, "still has a process of its own alive"), the four
refusals again publish nothing; released, a second run is admitted and makes both again.

Measured before commit as negative control A1 (the old ordering restored in `run_harness`, the
exclusion, the dirty-tree refusal and the include before `FreshPrepared::lease`): it fails at the
first case on "created .upstroke", "created .upstroke/.gitignore".

## `fn a_schema_4_resume_that_loses_the_worktree_lease_restores_no_exclusion_and_writes_no_include() {`

The resume half of the one defect, through `resume_with` with no `--private-root`, so the committed
locator is the only root authority. A run parks on its one question; its log's bytes are kept.
Under the lease this test holds, and on Unix beside a surviving reaper of the parked run, each of
the four states' resume is refused with nothing written in either root, no ref moved and no worker
or reviewer built; the log is the parked run's, byte for byte. Released, the resume is admitted,
appends `run_resumed`, and makes the exclusion and the include from absent. The owner and commit
proof before the include stays `a_schema_4_resume_proves_its_owner_and_commit_records_before_it_writes_its_include`'s.

Measured before commit as negative control A1 (`resume_harness` making the exclusion before
`ResumePrepared::verify`, byte for byte the file at `eceed606`): it fails at the first case on
"created .upstroke/.gitignore".

## `const DEFAULT_ROOT_LEASE_REPO: &str = "UPSTROKE_DEFAULT_ROOT_LEASE_REPO";`

The repository the default-root lease child runs in, named by its parent.

## `fn a_fresh_schema_4_run_over_the_default_root_that_loses_the_worktree_lease_creates_nothing_under_home()`

The same defect over the default private root (Unix, as `run_test_isolated` is). The default root
is `$HOME/.upstroke`, which no test may touch in the operator's home, so the parent makes a
directory of its own fixture tree the child's `HOME` and `USERPROFILE`, as
`a_schema_4_run_refuses_an_unspellable_default_private_root_before_it_creates_anything` does, runs
`default_root_lease_child` under `run_test_isolated` with a 300-second bound, and requires its
success and `1 passed`. The default root is the one branch whose root the include's publication
creates, and whose authorization (`RunnerResolved::authorize`) therefore follows the lease; this
is its witness that losing the lease creates no root, and that winning it creates and authorizes
one.

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

## `fn a_schema_4_resume_whose_router_cannot_read_its_log_refuses_before_publishing_anything() {`

PR12-INV5-RESUME-READ-ERROR-LEGACY-PUBLICATION and PR12-REG8-RESUME-READ-ERROR-LEGACY-PUBLICATION
(one defect), through `resume_with` with the explicit private root the run recorded. A run parks
on its one question, and the premises are read first: the census classifies its committed header,
its id resolves, and read normally its header routes the resume to the topology. Then, for each
of the four exclusion and include states (absent or changed, each), under the worktree lease this
test holds and with the router's initial read of the run's log failing, the resume is refused with
that read's own error after exactly one attempt at the read, and nothing was published: every
path, kind, mode and byte of the checkout outside `.git` and of the private root, and every ref,
as they were, and no worker or reviewer built. The no-publication comparison is asserted before
the refusal's identity, so it is the assertion the old fold fails.

The controls follow. With the run's owner record naming another run, and then with its commit
record digesting another first line, the unread resume is refused with the read's error and writes
nothing, and the same resume read normally is refused by the record (`records run id`, `says the
committed first line digests`) and writes nothing either: the read's refusal comes first, and the
proof still stands behind it. Read normally under the held lease, the resume is refused as already
driven and publishes nothing, and the log is the parked run's, byte for byte. Released, the resume
is admitted, appends `run_resumed`, and makes the exclusion and the include from absent
(`assert_admitted_and_published`). A genuine legacy run (`parked_run`) routes to the legacy lines
when read; with its log unreadable to the router it is refused with the read's error and nothing is
published under its root either; read again, `resume_answering` takes it to completion and the
legacy lines publish its include as before.

Measured before commit as negative control B1 (`route_resume` folding the failed read into
`Ok(None)` again, as at `8b7b22d4`, over the same read): it fails at the first case, exclusion and
include absent, on "created git", "created git/recorded-objects.gitconfig" under the private root.

## `const DEFAULT_ROOT_UNREAD_REPO: &str = "UPSTROKE_DEFAULT_ROOT_UNREAD_REPO";`

The repository the default-root unread child resumes in, named by its parent.

## `const DEFAULT_ROOT_UNREAD_RUN: &str = "UPSTROKE_DEFAULT_ROOT_UNREAD_RUN";`

The parked run that child resumes.

## `const DEFAULT_ROOT_UNREAD_DONE: &str =`

The line the child says on stderr once its last assertion has passed. The child returns early,
and still reports `1 passed`, when its parent's variables are absent (an `--ignored` run of the
whole suite), so its parent requires this line as well: a child that skipped cannot produce it.

## `fn a_schema_4_resume_over_the_default_root_whose_router_cannot_read_its_log_creates_nothing_under_home()`

The same defect with no `--private-root` (Unix, as `run_test_isolated` is). The legacy lines'
include goes under today's default root, `$HOME/.upstroke`, which no test may touch in the
operator's home; so, as
`a_fresh_schema_4_run_over_the_default_root_that_loses_the_worktree_lease_creates_nothing_under_home`
does, the parent parks a run under its explicit fixture root, makes a directory of its own fixture
tree the child's `HOME` and `USERPROFILE`, runs `default_root_unread_resume_child` under
`run_test_isolated` with a 300-second bound, and requires its success, `1 passed` and
`DEFAULT_ROOT_UNREAD_DONE`.

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

## `const RETAINED_HUSK_PRODUCER: &str = "engine::topology::create::tests::create_kill_child";`

The producer of the retained schema-4 husk the guards below resume: `create`'s own kill child,
whose `p5b` site is the one `kill_between_commit_record_and_run_started_retained_as_possibly_committed`
measures.

## `fn retained_p5b_husk(root: &Path) -> String {`

The retained husk P5b leaves, made by the actual producer rather than written by hand:
`create_run`, driven by `create`'s tests' driver, killed after `RunDir.PublishCommitRecord` and
before its first append, in the child [`RETAINED_HUSK_PRODUCER`] names, under the production
`HostRunner` with a 120-second bound (`create`'s own `spawn_and_wait` shape). The child lays out
`create`'s fixture under `root`: `root/repo`, which here is the Git checkout `temp_engine_repo`
already made (`create_private_dir` is `create_dir_all`), so the legacy lines' prelude would find a
repository there, and `root/private`, the husk's creation private root. It then asserts the shape
the producer's own test asserts: one run directory, `committed.json` in the private half, an empty
log, the census's `Husk`, and the disposition `Retained(PossiblyCommitted)` under that fixture's
own repository key. Returns the husk's id. It produces the P5b shape only. A failed first append
that `create`'s append-error protocol answers `RetainedPossiblyCommittedHusk` leaves, as that
protocol reads, the same two halves (its torn line truncated by `Event.OpenLog`); that producer is
not run here.

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

## `fn a_retained_schema_4_husk_is_refused_before_any_root_is_published() {`

PR12-REG9-HUSK-REFUSAL-LEGACY-PUBLICATION through `resume_with` naming an explicit root
(`private_root_for`, the fixture's `home`) that is not the husk's creation root. The positive
control runs first: a genuine legacy run (`parked_run`) still routes to the legacy lines and, with
its include absent, `resume_answering` takes it to completion and the legacy lines publish the
include as before. Then the husk is produced, the premises are read (the two roots differ; the
resolver refuses the exact id with its husk sentence naming the run), and for each of the eight
cases — the worktree lease free and held by this test, each exclusion and include state — the
resume changes nothing under the root it names, under the creation root, in the checkout outside
`.git` or in `.git` (every path, kind, mode and byte), builds nothing, and is refused with the
resolver's own refusal, byte for byte. The no-write comparisons precede the refusal's identity, so
they are what the old fold fails.

**The held lease is held, physically, throughout.** PR12-R12-WINDOWS-HUSK-SNAPSHOT-LOCK,
PR12-REG10-HUSK-SNAPSHOT-LOCK and PR12-INV7-HELD-LEASE-SNAPSHOT-WINDOWS (one defect, first in
`fbdd3e69`): the guard took the lease and then snapshotted `.git`, which reads
`.git/upstroke-worktree.lock` through a handle of its own. On Windows that read meets the
holder's `LockFileEx` range lock — Native16's `test (winguest)` failed in the snapshot's `fs::read`
with `os error 33`, though the log names no file, case or holder — and on Unix closing that
second descriptor released this process's `F_SETLK` lock (`rundir`'s lock notes: a holder must
never open its own lock file again), so the resume ran against a lease the operating system no
longer held, with only the in-process claim left. Now `.git` is snapshotted only outside the held
interval — before the lease is taken and after it drops — so nothing in this process opens the
lease file while it holds it; the lease file is created once before the loop and asserted present
(R25: created on first acquisition, never removed), so no acquisition below creates anything in
`.git`; and in every case [`assert_the_lease_is`] asks another process: before the resume (held:
refused, naming this process; free: taken and given back) and, held, again after the resume and
every observation made under the lease. Nothing is skipped on any platform, no production lock
changes, and every assertion the guard made it still makes, failures naming their case and path.

Measured before commit as negative control A13 (the router's three lines folding the resolver's
refusal into `Ok(None)` again, exactly as at `328ee6dd`): the legacy control passed, and the first
case failed on "created git", "created git/recorded-objects.gitconfig" under the named root. A
compiled and executed failure on the unwanted write, not a compile error or a timeout.

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

## `fn a_retained_schema_4_husk_resumed_over_the_default_root_creates_nothing_under_home() {`

The same defect with no `--private-root` (Unix, as `run_test_isolated` is): the case the finding
names, where today's default root is not the husk's creation root. The parent produces the husk,
makes a directory of its own fixture tree the child's `HOME` and `USERPROFILE`, runs
`default_root_husk_resume_child` under `run_test_isolated` with a 300-second bound, and requires its
success, `1 passed` and [`DEFAULT_ROOT_HUSK_DONE`]. No variable of this process changes.

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

## `fn a_budget_stopped_schema_4_resume_recreates_its_pruned_execution_root_where_it_recorded_it() {`

The positive control of PR12-INV6-RESUME-COMMON-DIR-REBIND for a root that finalization legitimately
pruned, on every platform. A run stopped at its budget (S9's fixture: one attempt per task, a
`0.015` run budget) is closed and finalized, and its emptied execution root is pruned; the premise
reads the recorded root absent. Resumed with a larger budget under the same Git environment, the
run is admitted although its root does not exist — the binding compares paths and requires no
existence — and recovery recreates the root, saying so ("the execution root was pruned when the run
last ended and has been recreated"), with no other namespace made under the private root.

## `fn manager_today(repo: &Path, run_id: &str) -> crate::workspace_manager::WorkspaceManager {`

`run_id`'s workspace manager as a resume derives it in this process: `WorkspaceManager::derive`
over the private root `RootDerived` reads from the run's record and the incarnation it recorded,
under this process's Git environment. Read-only.

## `fn retain_execution_root(repo: &Path, run_id: &str) {`

Leaves a finished run's execution root as a run keeps it when it ends before finalization's last
step: the scaffolding `WorkspaceManager::create_execution_root` makes, through [`manager_today`],
after asserting that this manager derives the root the run recorded.

## `fn copy_common_dir(from: &Path, to: &Path) {`

A second common directory this test owns: the checkout's Git directory copied file by file
(objects, refs, packed refs, logs and configuration) without its linked-worktree registry and
without the worktree lease's file. With `GIT_COMMON_DIR` naming it, Git keeps the checkout's own
`--absolute-git-dir` and answers this directory for `--git-common-dir`, which is the split the
finding needs, and nothing outside the fixture is copied or touched. A plain-Git fact the test
relies on, not a reproduction of anything by Git alone.

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

## `fn a_schema_4_resume_under_another_effective_common_directory_refuses_before_any_effect() {`

PR12-INV6-RESUME-COMMON-DIR-REBIND through `resume_with` on the host runner (Unix, as
`run_test_isolated` is; `GIT_COMMON_DIR` changes only in the children's environments, never in this
process's). In one checkout with no `GIT_COMMON_DIR`, four runs park on their one question (open,
resumable prefixes) and one completes, its pruned root then retained ([`retain_execution_root`]),
a valid terminal prefix with execution-root residue. Their owner, commit and log bytes are the
engine's own. The positive controls run first: a child with `GIT_COMMON_DIR` naming the checkout's
own common directory absolute, then one naming it relative (`.git`, given its meaning at the
checkout), then this process with none, each admitting its own open run and finalizing the finished
run at the root it recorded ([`assert_admitted_at_the_recorded_roots`]; the residue retained again
after each). Then [`copy_common_dir`] makes the other common directory, and the refusing child runs
with `GIT_COMMON_DIR` naming it. Each child has a 300-second bound and a completion sentinel.

Measured before commit as negative control B13 (`ResumePrepared::verify` discarding the binding's
answer with `let _ =`, so today's manager is adopted as at `328ee6dd`): the three positive controls
passed, and the refusing child failed at its first case on the wrong-root admission's writes — the
exclusion and the include published, the open run's log and report changed, and a namespace
`workspaces/<key>` of the other common directory created under the private root.

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

## `fn blocked_closure_plan() -> String {`

Four tasks: `t1`, whose body carries [`ESCALATES`] so its workers below the frontier end without an
edit; `t2` on `t1`; `t3` on `t2`; and `t4` on nothing.

## `fn dependent_rows(rendered: &str) -> Vec<&str> {`

The rendered rows of `t2` and `t3`, the direct and the transitive dependent.

## `fn declined_failure_run(repo: &Path, stub: &ObservedStub) -> RunReport {`

A schema-4 run through the public `run_harness` whose end-of-ladder question is declined by the
harness's scripted answer, so `t1` fails after its one attempt, under whatever `on_task_failure`
the repository's config holds.

## `fn a_completed_schema_4_run_reports_its_failed_tasks_dependents_blocked_live_and_replayed() {`

PR12-INV7-BLOCKED-TASK-REPORTED-QUEUED. With `on_task_failure = "continue"` the run completes with
`t1` failed and `t4` committed, and its final summary prints ``t2: blocked by `t1` `` and
``t3: blocked by `t2` ``, never `queued`: the derived Blocked view of the canonical completion rule.
The run's committed log, replayed through the fold's checked transitions with the frozen plan read
as `entry::frozen_inputs` reads it, is `Complete` with `t2` and `t3` still structurally `Pending`
— the run's own `report.json` says `pending` too — and projects the same rows. On the same log, the
prefix before the event that fails `t1` reports them `queued`, and the prefix ending with it
reports them blocked: that event, and nothing else, is what blocks them.

## `fn a_schema_4_report_keeps_queued_behind_a_parked_task_and_skipped_behind_a_halting_failure() {`

The controls. A task behind one parked on an unanswered question is `queued`: a question is not a
failure. And the same failing chain under the default `on_task_failure = halt` ends `Halted` at
`t1`, with `t2` and `t3` `skipped (run halted)`: a halted run keeps skipping what it did not reach.

## `fn execution_roots_under(private_root: &Path) -> Vec<String> {`

The run ids of every execution root under a private root's `workspaces/`, by name, so a root
spelled through a canonicalized private root compares equal.

## `fn a_fresh_run_on_an_unborn_branch_is_refused_before_it_creates_anything_and_a_born_one_runs() {`

PR12-INV7-PREPUBLICATION-EXECUTION-ROOT-ORPHAN through the public entry, on the route the finding
names: a clean repository on an unborn branch, its plan outside the worktree. The run is refused by
`git rev-parse HEAD` — the base commit it cannot record — and leaves no execution root of its own,
no run directory, no private half and no build, with the lease free; another run's root in the same
namespace, content included, is untouched. Before the repair the root was made before that read,
and stayed with no record naming it. Then the branch is born, and a fresh run on it completes and
records the root it created; the other run's root is still untouched.

## `fn a_configured_reviewer_whose_cli_is_absent_fails_creation_before_p5b_and_commits_nothing() {`

P12-W17 (D9). A configured reviewer whose CLI is absent fails creation before P5b, and nothing is
committed. Every recorded reviewer is probed at P4, so `run_with` refuses there, naming the
reviewer's program, after P3c claimed the run's execution root. The creator's cleanup leaves no run
directory, no private half holding `committed.json`, no execution root and no integration ref, and
no run was made without the reviewer. The control: the same configuration, with the reviewer's CLI
present, creates the run, records both agents as probed, and completes it.

This prose sat above the test in the source until the execution root moved into the create protocol
at P3c; the refusal used to be reported after P3b, and the root was made before creation began.

## `fn an_absent_alternative_reviewer_cli_refuses_creation_while_cross_family_is_unset() {`

CAP-CROSS-FAMILY-REVIEWER-CANNOT-BE-DROPPED at run creation, the half that refuses; its partner is
the next test. The tasks are implemented on the frontier rung, which the topology stub serves as
Claude Code's frontier model, so the primary reviewer, `claude-code/claude-opus-5`, is the model
that wrote the work, and with `cross_family` unset the run names another family's reviewer to take
its place. The harness drives `codex` and not `copilot`, so that reviewer is Codex's, and its CLI is
absent. Run creation probes every agent in the probe set it derives from `review_plan.agents()`, so
`run_with` refuses at P3c, before the commit record, naming the alternative's program as one that
could not be executed. The `codex` adapter was probed once and never built, and the creation left
nothing behind, by the same assertions as the configured-reviewer test above: no execution root,
no run directory, no private half holding `committed.json`, no integration ref.

## `fn an_absent_alternative_reviewer_cli_is_never_probed_once_cross_family_is_false() {`

The partner: the same plan, stub and absent Codex CLI, with `[routing] review = { cross_family =
false }`. The run is created and completes with both tasks committed. Its first logged line records
`claude-code` alone as probed, the `codex` adapter was neither probed nor built, and the primary
reviewed each task its own model implemented. The run's warnings hold exactly one that mentions
`cross_family = false`, and it names the primary.

The pair was measured before commit against two changes to the probe set in
`engine::topology::entry::prepare_fresh`. With `codex` added whenever the harness has it, this test
fails with the refusal the finding observed ("creating run … stopped at P3c", naming the absent
CLI), and its partner still passes. With the probe set taken from `review_plan.required_agents()`,
which leaves the alternative out, the partner fails because creation no longer refuses, and this
test still passes.
