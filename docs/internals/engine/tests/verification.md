# `src/engine/tests/verification.rs`

Extended notes for [`src/engine/tests/verification.rs`](../../../../src/engine/tests/verification.rs).

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

## `let (_tree, repo) = temp_engine_repo("maxparallelrefusal");`

The config refusal is only worth having if it lands before the run has
done anything an operator must undo. Pre-flight loads the config ahead of
the run id, the run directory, the run lock, and the branch — so a
ceiling this engine cannot honour leaves the repository exactly as it
was, rather than a husk under `.upstroke/runs` that `latest_run` then
reports on in place of the real one.

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
recorded as `PR4A-SPAWN-WITHOUT-AMBIENT` in `48f2af7a:reviews/FINDINGS.md` with an
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
