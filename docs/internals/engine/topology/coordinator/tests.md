# `src/engine/topology/coordinator/tests.rs`

Extended notes for [`src/engine/topology/coordinator/tests.rs`](../../../../../src/engine/topology/coordinator/tests.rs).

The code is the authority for what it does; this file is the whole of its prose but for the
concurrency protocol standards §10 places at its site. Each section
here is headed by the line of code it describes, spelled as it is in the source, so the heading is
the grep string that finds the code.

## Module

The scheduler first: `Seeded` is a splitmix64 stream; a `Scheduler` is a [`Quiescence`] observer
releasing the first held invocation, a seeded choice, or what a script chooses, and records the
widest set of invocations — and of slotted invocations — it saw granted at once. The tests hold
every invocation (`RecordingRunner::hold`) and answer it with the fixture's responder, so the order
of completions is the scheduler's and never the threads'.

A scheduler also owns the entry of every call into the runner. It puts its runner in late entry
(`RecordingRunner::enter_late`), so no granted call starts until the scheduler admits it, and it
admits each one in `granted` (`RecordingRunner::admit`), so every process starts while the
coordinator waits there and at no other time. At every quiescent
point it first checks that each invocation it is handed is inside the runner
(`RecordingRunner::inside`). A coordinator that acted again before a granted pipeline reached the
Runner therefore fails every scheduler-driven test at its first quiescent point, on any machine —
not only when a slow one happens to widen the window, as CI's did at `aea75a79`.

## `fn canonical(events: &[TopologyEvent], run_id: &str) -> Vec<serde_json::Value> {`

The packet's `canonical_trace_projection`, for schema-4 events: drop the run's identity and
environment and the scheduling configuration, keep everything else, and label every commit SHA by
first appearance (tree OIDs literal). The labels are numbered over the slice it is handed.
`attempt_level`, ST-08's independent half, hands it one key's attempt-level trace at a time, so there
they are that key's; every other caller, the chain comparisons among them, hands it the whole log, so
there they are the log's.

## `fn scheduled_here(every: std::ops::Range<u64>, on_windows: &[u64]) -> Vec<u64> {`

The schedules a seeded test runs on this platform (the record's R-AY): every seed of `every` on Linux
and macOS; on Windows only `on_windows`, which must be a non-empty subset of them. A seed is one
schedule of the coordinator's logic, which has no platform branch and is reproduced exactly on every
machine (R-AD); what Windows adds is its own I/O — process creation, Git for Windows, its paths —
which every schedule exercises alike, and there a seeded test costs 37 to 44 times what it costs run
alone on Linux (the CI guest's reconstructed durations; each schedule starts two hundred to three
hundred Git processes, nearly all of them the manager's Git effects). So Windows runs the fewest schedules a test's assertions compare: one per
configuration, two where a test asserts equality across seeds, and for a claim made of *some* seed
the seed that makes it (the Linux runs name it). Tests whose behaviour differs on Windows — the kill
matrices, the two-process tests, the container runner, the scripted shapes — run in full everywhere.

## `const ONE_SUCCEEDING_ATTEMPT: [&str; 4] = [`

What a key whose one attempt succeeds projects, in order: one `task_dispatched`, one
`attempt_started`, one `candidate_prepared` and one `task_candidate_created`, and no
`attempt_finished` (`design/26_design_merge_queue_protocol.md`: `candidate_prepared` is "the
**sole** successful settlement for an attempt that produces a candidate … `attempt_finished` is not
also emitted for that attempt"; `settle::settle_failed` builds an `AttemptFinished4` only for the
other outcomes). `attempt_finished` stays in `ATTEMPT_LEVEL`, so a key that has one projects it
(`a_keys_attempt_level_projection_depends_on_its_own_trace_alone`).

## `fn attempt_level(events: &[TopologyEvent], run_id: &str) -> BTreeMap<u64, Vec<String>> {`

ST-08's per-key projection for independent tasks, which compares "only already-defined projection
fields per key": the events of `ATTEMPT_LEVEL`'s five kinds, selected from the raw log by their typed
kind, grouped by task key in log order, and each key's whole trace handed to `canonical` on its own.
A key's commit labels are therefore numbered within its own trace — in these tests its base is
`commit-0` and its candidate `commit-1` however the other keys and the integrations interleaved with
it — while every field, literal tree OID and same-or-different commit relationship inside the trace
is kept: the labels restart per trace, never per event or per field. They say nothing across keys.
The shared base (`dispatched_bases`) and the integration order (`task_candidate_created` against
`merge_prepared`) are separate assertions in the tests that claim them, and neither is read from
these labels.

Labels numbered over the whole log would number each key's candidate by the order every key's
candidate, and the integrations between them, reached the log, which is the schedule a permutation
changes: two seeds whose candidates reached the log in different orders project alpha's candidate as
`commit-4` and as `commit-2`, every other field equal. An earlier selection compared nothing at all
(G6-ST08-EMPTY-PER-KEY-PROJECTION): it kept serialized events whose top-level `kind` field named one
of the five kinds, and the body is tagged `event` (`#[serde(tag = "event")]`), so every key projected
an empty list, every per-key comparison held vacuously, and Gate 6's `projection/independent` export
at `e0932045` reported 48 digests of empty input as equal.

## `fn attempt_level_problems(`

The nonvacuity check every `attempt_level` caller asserts empty before it compares seeds: each
expected key projects exactly `ONE_SUCCEEDING_ATTEMPT`, read from each projected value's `event` tag,
and no other key is projected. An empty projection fails it for every expected key.

## `fn halt_cancels_in_flight_attempt_at_width_three() {`

T-ATTEMPT at width three: a decline halts the run while alpha is at its gate and beta at its worker;
both are terminated, each released once after its termination, and the closure appends one
`attempt_interrupted` per attempt, then reclaims its snapshot and worktree, before `run_finished`.
`Watching` records the execution root's intents after every append, which is how the order of
terminal and reclaim is read. With `halt_interrupts_verification` (T-VERIFY: a halt inside
`verify`, settled by `merge_verification_interrupted`, the pin deleted and this sequence's staging
and snapshots reclaimed before the end), the phase-3 test that held the closure's refusal is gone.

## `fn the_overshoot_notes_bound_the_remaining_spend_of_every_pipeline_live_at_the_stop() {`

Pins the `Budget overshoot` section above, as `run/tests.rs` pins its notes: it must state the
remaining spend, the review passes and re-asks, and that no count of invocations bounds it, and the
retired reading — one invocation per live pipeline — must not come back. Red against the notes at
`a9e88039`.

## `fn settled_but(run: &mut TopologyRun, held: &[InvocationId]) -> Result<(), String> {`

The ledger after the command ended: every registration settled exactly once but the ones in `held`,
whose processes the Runner could not establish as ended. Those stay running and keep their slot
pairs for the rest of this process — R3 and R4 release them at process death — and nothing is
pending and no settlement was a duplicate. This is what "the ledgers balance" means once an
invocation is held (round R1, `R1-CONC-1`).

## `fn sibling_parked_on(`

The sibling repair of `root` that the fold's own sibling-repair tests build
(`topology/fold/tests/questions.rs`): spawned, dispatched and started, then parked on an embedded
question — four events a test plants through `Release::Append`, because the live engine of this build
spawns a repair only from a rejection and never runs two members of one lineage at once.

## `fn bounded(what: &str, body: impl FnOnce() + Send + 'static) {`

Run a scenario on a thread named after the test's, and wait for it a bounded time: the regression the
two tests below guard against is a coordinator that waits for ever, and it must fail the test, not
hang the suite. A scenario that never ends leaves only its own fixture's thread blocked. Since
follow-up B's repair round 6 (I2-5) every R1 witness and each witness of that round runs its
coordinator call through it, so a mutation that loses a wake fails its witness within `BOUND`
rather than wait for an external timeout.

## `struct DecliningAtPoll(std::sync::Arc<std::sync::atomic::AtomicUsize>);`

An answer source that declines at the n-th poll after it is armed with n: each admission pass polls
an open question once, so a scheduler that arms it with 2 as it releases a pipeline's last process
places the decline in the pass after that pipeline's completion is applied, the end having been
applied in the pass before.

## `fn container_coordinator_child() {`

The second coordinator of the two-process tests (R-AM): this test binary run again, `--exact` and
`--ignored`, by `scaffold::Served`, with its stdio linked to the parent (`ParentSide`) and its
container runtime the parent's (`LinkedRuntime`). Its role is named by the environment:
`fresh` creates a durable width-3 run under a ULID-shaped incarnation, reports its fixture root and
runs its coordinator until it is killed (`run_held`: every container holds, since nothing in the
parent completes a child's container); `resume` adopts a root, resumes it through the frozen
recovery order over the parent's runtime and the production `LockProbe`, with a pre-flight whose
shell probe is a container, reports what recovery settled and runs until killed; `census` is a
fresh write command in a second repository — a read-only pre-lock refusal first (an unknown run id),
then the worktree lock, then `startup_census` — and reports the census, then, when asked, takes its
own run lock and runs one agent-probe container that mounts the shared credential volume; `owner`
holds a run lock of its own and two probe containers until it is killed, which is how a dead owner
comes to exist. `hosted` (Unix) is a coordinator whose pipelines run real host processes
(`HostHeld`).

## `fn broker_is_empty(run: &mut TopologyRun) -> Result<(), String> {`

R3, R4 and R13 at a coordinator's start: nothing registered, running or pending, no pair held or
waited for, no entitlement held, no reservation ever taken or cancelled, no duplicate counted.

## `fn hosted_linked_registry_child() {`

T-ENV-TOPO's child. It adopts the fixture (`Fixture::adopt_based`), resumes as `inc-2` at width two,
and drives the run with `run_concurrently` over one host runner for both the coordinator's seams and
the pipelines'. Under C4's absolute value that runner is `HostRunner::new()`. Under C5's relative
value it is a host runner that holds the run's repository (`ObjectGraph::RecordedIn` over
`ManagedRepository::new`, its include written at `<root>/recorded-objects.gitconfig`): under option
(C) a role in a work tree the engine made is given the common directory its runner holds in place
of an inherited relative value, and a runner that holds none refuses that role before it starts.
`f_hosted_runner` builds that runner, which T-NOREQ's child takes too.

**On Windows** the seams' and the pipelines' adapters are `FHostedStdinPromptAdapters`: the
scaffold's answering adapters, with a review's prompt on its stdin alone. `AnsweringAdapter::build`
also passes the prompt as an argument. There the roles are batch shims, and std refuses to start a
batch file with an argument holding a newline, which a prompt holds. Every shipped adapter sends the
prompt on stdin only, and the stand-in reads neither copy, as the script reads neither.

The run must complete. Its steps reach each placement:

- **Placement 3.** Alpha's first attempt's empty diff is a failure that placement 3 judges, so
  completion shows that placement 3 accepted it.
- **Placement 2.** The gates' verdicts and the review passes' verdicts, the verification's included.
- **Placement 1.** The capture's reads.
- **C1-AP.** The review-input reads, the verification's in its staging tree included.

**The verification, from the events.**
- **Exactly one `merge_verification_started`:** sequence 1, stale-clean.
- **The first `merge_prepared`** is sequence 0's: fast, with no verification.
- **The second** is sequence 1's: stale-clean, for the same candidate, on a passing verification record
  of one gate set and one review.
- **Both tasks merged:** one merge of each, and two `task_merged`.

**The role log** must hold exactly the placements `f_hosted_placements` names, each in this
incarnation's instance of its slot, and no other line:
- alpha's worker twice in its generation's slot, and beta's once;
- each working attempt's gate and review in its own snapshots: alpha's attempt 2's and beta's
  attempt 1's;
- sequence 1's verification gate in `s1-integration` and its review in `s1-review0`.

The counts bind each line to its phase, so no attempt's run counts as the verification's. Under C5,
every logged directory must name the run's common directory by the relative value. **The run's end:**
finalization, which `run_concurrently` runs before it answers `Finished`, removed the execution root.

**The fixture's lifetime.** The parent owns the scratch tree: its fixture holds the tree's guard, which
reclaims the root when the parent returns, after its own assertions. The child reaches the same root
through `Fixture::adopt_based`, which is for a root a dead child left. An adopted fixture's `Drop`
reclaims its root, so a child that returns normally would remove the parent's tree. After its last
assertion the child releases the run and every other part of its environment as usual, and forgets
only the adopted fixture, as P-3's `late_add_child` does.

**Predicted:** red at `f1842dcd` (the overlay) and under the reversal, at the first guarded read. At
`cbc0070e`, before the store pin, red at the first removal, the gate snapshot's: the removal's walk
from the common git dir met the store's link (HF-ENV, kept as observed).

**Observed at `af1a3dbd`, with the pin, and corrected since.** The one-task runs completed (HF-PIN,
kept as observed): `Complete`, with the execution root removed. Their role log held the worker twice
and one gate and one review pass, both of attempt 2, so the gate count failed.
- **Why.** One task merges by the fast path. When it merges, the integration head is still its
  candidate's base (`decide_pausing` in `integrate.rs`), so `prepare_fast` records no verification,
  and no verification gate or review runs.
- **The first diagnosis, qualified.** It took the reviewer stub's missing stdout verdict for the
  cause. The run's adapters answer with their own verdict, so printing one changed nothing.
- **The correction.** The two-task plan above makes the run verify an integration, and binds the role
  log to each phase's placements.

**Observed at `d1bfb37a`, and corrected since.** Both children passed (HF-PIN, kept as observed): the
completion, the verification's events and every placement held. Each parent then failed its first
end-state assertion, at `coordinator.rs:15888`: "the run's store is still the link it started under".
- **What the static trace shows.** The child's adopted fixture is dropped on the child's normal return,
  and reclaims the root the parent goes on to assert on. Whether that is what the parent met was not
  traced at run time.
- **The correction.** The lifetime above: the child no longer reclaims the parent's tree.

## `enum FNoRequest {`

T-NOREQ's three runs of a review pass that hands its runner no request, each over T-ENV-TOPO's linked layout
and the host runner. Root's `F-REGULAR2-REPAIR-DISPOSITION-20261028` directs them, under its tracking ID
`F-R2-NO-REQUEST-REGISTRATION`, which is not round 1's F-R2.

- **`Outage`,** the witness. Sequence 1's review pass finds a directory at its permission file's path, and
  ends unavailable before its first request.
- **`Damaged`,** the negative. The same pass, with its snapshot's registration damaged first.
- **`Opaque`,** the control. Every review request is admitted, reported `Unknown` by its runner, and never
  started.

## `struct FPermitted {`

The scaffold's passing adapter for one id, with the production adapter's `materialize_permissions` for that
id: `ClaudeCodeAdapter`'s for `AGENT`, `CopilotAdapter`'s for `REVIEW_AGENT`. Each writes
`<settings>/<stem>.json`, so a directory planted at that path fails the write, as the regression lens's
trigger does. Everything else is the scaffold adapter's. `FPermittedAdapters` holds one for each id.

## `struct FDamagingReviews;`

`LegacyReviewPasses`, which first removes `HEAD` from the registration entry of sequence 1's pass 0 snapshot
(`f_entry_of`). The snapshot is damaged after it is made and before the pass runs.

## `struct FOpaqueReviewer {`

The host runner for every role but the reviewer. A review request is reported `Unknown`, the trait's default,
and its run returns a `NeverStarted` error without starting a process. `Registering` admits the request and
keeps that report, and the pass reports zero processes, so the reported count and the registering history
disagree.

## `fn f_hosted_no_request_child(case: FNoRequest) {`

T-NOREQ's child. Like T-ENV-TOPO's, it adopts the fixture and resumes as `inc-2` at width two.

- **The trigger.** It plants a directory at the run's `settings/integration-s1-review.json`.
- **The seams.** It drives the run with `run_concurrently` over T-ENV-TOPO's host runner for the layout
  (`f_hosted_runner`), or `FOpaqueReviewer` over it for `Opaque`. Both the coordinator's seams and the
  pipelines' take `FPermittedAdapters`, and for `Damaged` both take `FDamagingReviews` as their review passes.
- **Under C5,** that runner holds the run's repository. Over `HostRunner::new()`, option (C) would refuse
  every role in a work tree the engine made before it starts, and the run would not reach the pass. Every
  logged directory must name the run's common directory by the relative value.
- **At the end** it forgets the adopted fixture, as T-ENV-TOPO's child does.

**The production path the witness reaches:**
1. The pass's permission write fails at `run_review`'s permission step. The pass ends `Unavailable` with no
   request, so its `Registering` keeps no report.
2. The process-count check passes, zero against zero. Placement 2 checks the pass under the manager's own
   builder.
3. The verification's judgement is the reviewer's outage. `integrate.rs` sends that down its ordinary
   unavailable route: deferred while `max_defers` allows, and then parked.

**`Outage` must complete,** with the execution root removed.
- **The outage:** exactly one `merge_verification_unavailable`. It is for sequence 1, an infrastructure outage
  of kind `review_unavailable`, deferred once. Its one charged review pass is unavailable, with no cost.
- **What follows:** a `defer_wait_elapsed`, which the scaffold's `NoSleep` waits at once. Then the same
  candidate's stale integration is verified again, under a new sequence, and its merge is prepared on that
  verification: one gate set and one review, passing.
- **The accounting:** two `task_merged`, and every request the run admitted was ended (`invocations_balance`).
- **The role log:** T-ENV-TOPO's placements, with no line in sequence 1's review snapshot, and the gate and
  review of the sequence verified again.
- **The parent** asserts T-ENV-TOPO's end state.

**`Damaged` must be refused,** with `RegistryRefused` naming the verdict of review pass 0 and a `HEAD` that
cannot be read.
- There is no `merge_verification_unavailable`.
- Sequence 1's verification began, and its gate ran.
- Nothing ran in its review snapshot.

**`Opaque` must be refused,** with `RegistryRefused` naming the verdict of review pass 0 and the original
rule's text: `commondir` does not name the run's common git dir.
- No review process started.

**Predicted at `a9d91de7`,** where an empty list reads as `Unknown`. `Outage` is refused there by the original
rule, so its run fails and the child with it. `Damaged` and `Opaque` hold there as at the repair: the damage
is found before the rule's comparison, and the opaque request's report is the runner's.

## `fn hosted_no_request_outage_child() {`

T-NOREQ's child, for `Outage`.

## `fn hosted_no_request_damaged_child() {`

T-NOREQ's child, for `Damaged`.

## `fn hosted_no_request_opaque_child() {`

T-NOREQ's child, for `Opaque`.

## `fn stranded_child(parent: &crate::engine::topology::scaffold::ParentSide) {`

A width-three coordinator over the parent's fake that reports its error return, drops every
handle on its runner, reports that, and then waits for a file the parent writes before it exits
of its own accord, so the parent can observe it alive past its last handle.

## `struct TearsAForeignRegistration {`

PR11 review round 8's witness (`R8-CONC-1`), the verification face of
`PR11-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY`: a review-input policy that, at the
verification of a candidate (the staging worktree under `merge/`), plants another process's
registration half written in the shared store — `HEAD` and `gitdir` written, `commondir` opened
and empty, the state `git worktree add` passes through and leaves when it is killed there. Once:
the first verification only. With `finishes`, a thread plays the other process finishing its
write, but only after a registry access of this run has failed on it at least once — the
`CONTENDED_ATTEMPTS` handshake of `workspace_manager::tolerant_registry_access`'s contract
(`48f2af7a:reviews/2026-10-01-pr11-follow-up-b-record.md` §6.4) — so the witness measures a retry and
never a store that happened to be whole. Through the fixture's primitives: a topology module
may name no `std::fs` write.

The planted `gitdir` spells its path as Git writes it (`workspace_manager::fixture::as_git_writes_it`,
the host runner's Git-for-Windows form with `/` separators, and the path's own bytes elsewhere), as
every registration #329's tests plant does (repair round 3, the record's §9.13, R6).

The policy owns its writer (repair round 3, R3). The verification that plants the tear stores a
`ForeignWriter` once. `finish` cancels it and joins it, and
hands back what it returned: `Ok` only when it finished the registration after an access failed on
it. `settle(false)` joins it without a cancel, so the join waits for the handshake or the writer's
watchdog. The writer looks for the handshake every millisecond until its `watchdog` (`BOUND`, as
`tearing` makes the policy); when the handshake never arrives it writes nothing and returns why.
The drop cancels and joins a writer no test finished, and fails with what that writer returned,
saying it on stderr instead when the thread already unwinds, so nothing the writer does lands
after the fixture is reclaimed. Before, the writer's handle was discarded: it wrote `commondir`
when its watchdog expired whatever had happened, and a writer that outlived its test recreated a
directory its fixture had already reclaimed.

## `struct ForeignWriter {`

`TearsAForeignRegistration`'s writer: the sender that cancels it, and the handle `finish`, or the
policy's drop, joins.

## `fn tearing(`

The policy and the administrative directory it plants, named `name` in the run's own store.

## `enum TearAt {`

Where the R1 witnesses tear a foreign registration (the follow-up B record's §9.13): when an event
is folded, or just before the n-th registry access the coordinator's thread starts after an event
is folded (`workspace_manager::fixture::before_registry_access`), which names an access no fold
precedes directly.

## `struct Prober {`

The writer that finishes the tear, owned by `TearHeld`: the sender that cancels it and the handle
its `finish` or its drop joins (standards §10). What it reports, and after which sample, is under
`fn tear_sampling_every`.

## `enum Wakes {`

What lets the prober finish the tear: an invocation entering the runner (a pipeline was granted),
one leaving it (the observer released a held invocation inside the wait, and its end was applied),
or a registry access answering `Attempt` (the width-1 control, where no pipeline is there to serve).
`Never`: nothing does; the prober only waits to be cancelled, and the witness finishes the tear
itself or leaves it (round 6's shutdown witnesses).

## `enum Torn {`

The two torn shapes a witness plants: an empty `commondir`, on which Git's enumeration dies, and a
registration that is `locked` and has no `gitdir`, which the removal scan cannot bind under
`WriterProof::Unknown` and the enumeration passes over. Each is a shape a killed `git worktree add`
leaves.

## `struct Plant {`

The tear `TearHeld` plants in the run's own store — `HEAD`, a `gitdir` spelt as Git writes it and an
empty `commondir`, or `locked` alone — and the prober it starts, which completes the registration
once what it waits for happened after the tear and otherwise writes nothing.

## `impl Plant` › `fn finish_tear(&self) {`

Complete the registration the tear left, as its writer would: the whole registration for the
`locked` shape, then `commondir`, then the lock removed. The prober calls it, and a shutdown witness
calls it from inside the wait, right after it injects the shutdown.

## `impl Plant` › `fn tear_sampling_every(&self, every: Duration) -> Prober {`

Plant the tear and start its prober. The prober waits at most `every` at a time, a cancel ending the
wait early, and after each wait it samples what it waits for (`Wakes`). Every report follows such a
sample: `Ok`, the tear finished, when the sample shows that what it waits for happened after the
plant; otherwise the cancel, or its deadline once `BOUND` has passed. A witness cancels it only after
the call it watches has returned, so anything that call counted happened before the cancel was sent,
and the sample after the cancel sees it however long the wait was. `tear` samples every millisecond.

Before repair round 7 (the follow-up B record's §9.17) the prober sampled before each wait and
reported a cancel without sampling again, so what happened during its last wait was reported as
nothing. The integration witness below counts its attempt about 1.2 ms before its cancel on Linux
(the coordinator's first backoff, then the return), longer than one millisecond-sampler period, so
it passed there. A wait that ends only at Windows' clock tick, commonly 15.625 ms, mostly misses
such a window: that is the reasoned cause of `test (winguest)`'s failure of the witness at
`a337efa7`, and a 15.625 ms sampler reproduces it on Linux. The witnesses whose access cannot pass
until the prober has finished the tear (the R1 witnesses, the width-1 control, and the closure and
finalization waits) were never exposed: their prober has reported before the run they drive ends.

## `impl Wakes` › `fn not_seen(self) -> &'static str {`

What a prober's report says it did not see, by what it waits for. Before repair round 7 every
report said that no invocation reached or left the runner, whatever the prober waited for.

## `type FoldAct = (fn(&TopologyEventBody) -> bool, Box<dyn FnMut()>);`

An act `TearHeld` runs once when an event is folded, besides the tear: a broken worktree before a
retry, a file at a destination before an add.

## `const WITNESS_REGISTRY_DEADLINE: Duration = Duration::from_secs(10);`

The registry access deadline a tear witness gives its own repository
(`crate::workspace_manager::RegistryDeadline`): the production deadline's ten seconds rather than
the suite's 500 ms. Every such witness ends its access's wait itself -- its prober finishes the
tear, or its scheduler injects a shutdown -- once the coordinator reaches the point the witness is
about, and how many Git processes run before that point is the platform's to say: on the Windows
leg under the suite's load the three before a dispatch's intent witness could inject its shutdown
took longer than 500 ms, and the access refused first. Fix P (P-all), follow-up C's round 5, in this
change's B4 round (the follow-up B record's §9.22); the Windows reading is a Linux stand-in's. The
four witnesses that plant their tear without `TearHeld` hold it themselves (the B14 round, §9.31).

## `struct TearHeld {`

The run's hooks with one tear planted on the coordinator's thread at a `TearAt`, its prober owned
and joined, and an optional `FoldAct`. `finish` cancels and joins the prober and hands back what it
returned: `Ok` only when it finished the tear after what it waited for. The drop does the same for a
prober nobody finished. At `54a1ff14` the coordinator slept through every such access, so the prober
never saw anything and the access refused at its deadline. While it lives it holds a
`RegistryDeadline` giving its repository's accesses `WITNESS_REGISTRY_DEADLINE`, so every tear
witness's access waits to the production length, not the suite's 500 ms, before the witness's own
act ends the wait (fix P). Everything before the wait still counts against that deadline: an access
asks for the wait after a failed attempt only if time remains when it reads its clock after that
attempt's veto and count. If the deadline has passed by then, wherever the time went (the
platform's Git, the veto, the count, or a descheduled thread), it asks for none: it makes its final
attempt, or refuses if the failed attempt was already its final one (§9.32, §9.34).


## `impl TearHeld` › `fn tearing(&mut self, torn: Torn) {`

The shape the tear takes.

## `impl TearHeld` › `fn planted(&self) -> impl Fn() -> bool + 'static {`

Whether the tear has been planted, for a release order that waits for it.
## `fn two_independent() -> [WideTask; 2] {`

Beta, key 0, then alpha, key 1: in the order the admission dispatches them.

## `fn attempt_started_of(key: u32) -> fn(&TopologyEventBody) -> bool {`

The predicate of a task's `attempt_started`.

## `fn task_dispatched_of(key: u32) -> fn(&TopologyEventBody) -> bool {`

The predicate of a task's `task_dispatched`.

## `fn beta_settles_while_alpha_holds_its_first_gate(view: &Quiescent<'_>) -> Option<Release> {`

The release order of the observed witnesses: alpha's worker first, then beta's invocations while
beta is live, and alpha's held first gate only once beta is not — inside the wait of an access made
for beta, where alpha's second gate is then asked for and granted, and enters the runner.

## `fn candidate_created_of_beta(body: &TopologyEventBody) -> bool {`

Beta's `task_candidate_created`.

## `fn merge_prepared_of_beta(body: &TopologyEventBody) -> bool {`

Beta's `merge_prepared`.

## `fn retained_attempt_finished_of_beta(body: &TopologyEventBody) -> bool {`

Beta's `attempt_finished`.

## `fn alpha_waits_for_the_tear(planted: impl Fn() -> bool + 'static) -> Script<'static> {`

The release order of the repair witnesses: as `beta_settles_while_alpha_holds_its_first_gate` once
the tear is planted, and before that alpha's worker and beta's invocations only, releasing nothing
otherwise, so a first revalidation that waits out its deadline does not spend alpha's held gate.

## `fn served_while_a_repair_waits(`

The witness of a revalidation's repair arm (census A6, B1 and B2: the torn plan and its scans) and
of the verification's own read. With `residue`, a dead add's residue of a slot an intent names — its
intent written before the run, its registration with an empty `commondir` planted at the fold — makes
the first revalidation refuse at its deadline, the repair's plan find it, and its forced removal
remove it; the tear is planted at the access the test names. The residue is gone afterwards.

## `fn a_pipeline_is_served_while_an_intent_removals_repair_plan_waits_on_a_torn_registration() {`

The reclaim's intent removal: its repair's plan (the torn plan, R-X alone) meets the tear.

## `fn alpha_waits_for_gammas_integration(view: &Quiescent<'_>) -> Option<Release> {`

The release order of the stale witnesses: everything but alpha's held worker while gamma is live,
and alpha's worker once gamma is not — inside the wait of an access of gamma's integration, where
alpha holds no snapshot, so the stale arm is not deferred (R-W).

## `fn alpha_waits_for_gammas_merge(view: &Quiescent<'_>) -> Option<Release> {`

The same, holding alpha's worker until gamma's `task_merged`.

## `fn candidate_created_of_gamma(body: &TopologyEventBody) -> bool {`

Gamma's `task_candidate_created`.

## `fn task_merged_of_gamma(body: &TopologyEventBody) -> bool {`

Gamma's `task_merged`.

## `fn merge_rejected_of_gamma(body: &TopologyEventBody) -> bool {`

Gamma's `merge_rejected`.

## `fn repair_dispatched(body: &TopologyEventBody) -> bool {`

A repair's `task_dispatched`.

## `fn a_shutdown_injected_once_the_tear_stands(`

The scheduler of a shutdown witness (I2-1): at its first quiescent point after the tear is planted —
which is inside the wait of the access that met the tear, since nothing else asks the observer
before that access returns — it records the log's length, injects `Shutdown` and, when the witness
asks, finishes the tear, so that a transition that ignored the shutdown would find its access passed
and carry on. Before that point, and after, it orders as the witness's own script does.

## `fn published_as_the_log_authorizes(wide: &Wide) {`

The integration ref is where the log's last `task_merged` (or the run's base) puts it: nothing was
published that the log does not record.

## `fn of_key(events: &[TopologyEvent], kind: &str, key: u32) -> usize {`

How many events of `kind` name task `key`.

## `fn coordinator_of<'s>(`

A coordinator over a run, with its own channels, runtime and timer, for the witnesses that drive one
transition directly, as the reviews' witnesses did.

## `struct RequiresAFailingFilter {`

A review-input policy that, at the first verification, makes every checkout of the repository
run a required smudge filter that fails (`info/attributes` and the repository's configuration,
through the fixture's `git`): a snapshot's checkout then fails after Git took its destination
over, as a checkout that cannot be made does.

## `struct PlantingAtTheSnapshot {`

Plants one of #329's own faults just before the verification's fourth snapshot add, the third
reviewer's, by asking the planter for the review-input problem of a path under `merge`, as the
verification asks it: the gate's and the first two reviewers' snapshots are made and their passes
charged first. The planter is #329's `TearsAForeignRegistration` (T2′'s registration that stays
torn) or `RequiresAFailingFilter` (T15's required filter that fails).

## `const RECORD_KILL_TAG: &str = "record-kill";`

The tag `charged_record_kill_child` builds its fixture under, short on purpose like the other kill children's
tags. The child's temporary directory is a `kill_dir(RECORD_KILL_TEMPORARY)` tree its parent holds
(`scaffold::child_temporary_of`), so the fixture lies one scratch tree below the temporary directory, and its deepest
worktree, the first verification's reviewer snapshot, has a `$GIT_DIR` of the temporary directory plus 153 bytes plus
this tag. The tag was `coordinator-o14-record-kill`, 27 bytes: under the self-hosted Windows guest's 41-byte
`C:\Users\Administrator\AppData\Local\Temp` the path was 221, one over Git for Windows' 220
(`WINDOWS_GIT_DIR_BUDGET`), so the child's first add was refused there, it exited 101 in its setup, and both
charged-record kill tests failed with the kill never armed (`PR334-RECORD-KILL-CHILD-GITDIR-OVER-THE-WINDOWS-BUDGET`).
At 11 bytes the path is 205. The test below keeps it within a stated margin.

## `fn a_registry_refusal_after_two_paid_passes_is_recorded_with_both_at_width_one() {`

The same refusal at width 1, through `step`: the record is the step's last append, and the resumed
step verifies the candidate again under the next sequence.

## `fn stopped_inside_a_retrys_wait(`

The two `O14B2-1` witnesses' run, as #329's `stopped_in_its_wait` runs its own. A registration is
torn when the retried task's retained `attempt_finished` is folded, so the retry's access fails on it
and waits, and the shutdown is injected inside that wait, with the tear finished at once so that only
the stop keeps the access from succeeding. The run ends on the shutdown. It checks that the access
failed on the tear first and that no wait slept on the coordinator's thread, and returns the
command's error and the log's length at the injection.

## `fn the_record_past_a_stop_leaves_the_run_stopped_for_what_follows() {`

`record_charged_past_the_stop` lifts the stop for its one offer only. On a run a wait has stopped, the
helper is called, and the stop still holds after it: a dispatch begun next is refused by the run's
emitter and nothing is appended. The refusal keeps #329's words, "nothing further is appended in this
process", then names the one append made past the stop, the record of the review passes a
verification had charged, and then the stop.

## `fn f_entry_of(checkout: &std::path::Path) -> PathBuf {`

The registration entry a checkout's `.git` pointer names, read from the file (it works while the entry itself is gone).

## `const BOUNDARY_ROOT: &str = "UPSTROKE_TEST_R5_BOUNDARY_ROOT";`

The scratch root of the run whose `derive` boundary [`relative_name_derive_boundary_child`]
checks. A child started without it returns at once.

## `fn relative_name_derive_boundary_child() {`

The child of [`a_relative_object_directory_refuses_derive_with_the_resumes_arguments`]: it passes
only when `derive`, called with the resume's arguments, refuses, naming the variable and saying to
export it as an absolute path.
