# `src/runner/container/fake.rs`

Extended notes for [`src/runner/container/fake.rs`](../../../../src/runner/container/fake.rs).

[Source on GitHub](https://github.com/sourcemaps/upstroke/blob/master/src/runner/container/fake.rs).

The code is the authority for what it does; this file is the whole of its prose, moved out of
the source verbatim. Each section is headed by the line of code the comment sat above, spelled
as it is in the source, so the heading is the grep string that finds the code.

## Module

The deterministic test substrate: the fake container runtime, the fake owner
liveness probe, a recording hooks observer, and the Docker gate.

`decisions.tests_acceptance.determinism` specifies the fake **exactly**:

> a fake container runtime with (6) owner labels, incarnations, liveness
> simulation, (1) an image table keyed by immutable id with references and
> digests, (2) a mutable tag table (a reference can be moved to another id),
> (3) per-container reported image ids with substitution injection, (4)
> volume presence toggles, and (5) an availability toggle for ST-16 and
> ST-20 plus Docker-gated real runs

All six are here and each is exercised by at least one test that fails
without it; `runner::container::tests` says which, by name, in the doc
comment of each.

#### The correlated-fixture trap, which this module is built around

If a helper set a container's **reported** image id from the id it was
**created from**, no test in this slice could construct a substitution and
`substituted_image_id_refused_before_start` would be green because it could
not be written. So [`FakeRuntime::seed_container`] takes the two as
**separate arguments**, [`FakeRuntime::substitute_reported_image_id`] is the
injection point at create, and
`runner::container::tests::the_fake_can_report_an_image_id_that_differs_from_the_one_create_asked_for`
proves they can differ.

## `#![forbid(`

`PR6-LANEF-004`: the Container funnel's module-level allow is an INNER
attribute, and a Rust lint level is scoped by the MODULE TREE rather than by
the file, so every out-of-line child of `runner::container` inherited it --
measured, a `ContainerRuntime::start` planted in a child module passed
`cargo clippy --all-targets --all-features -- -D warnings`. Re-denying here
is what makes `decisions.effect_site_inventory.mechanism` (1)'s BUILD error
true of a lane's module, which is the leg the source census cannot supply.
Enforced for every file in this directory by `runner::container::tests::
every_child_module_of_the_container_funnel_states_its_own_lint_level`.

## `pub(crate) struct FakeImage {`

---------------------------------------------------------------------------
The fake runtime
---------------------------------------------------------------------------

## `pub(crate) struct FakeImage {`

One image in the table.

## `pub(crate) struct FakeImage` › `pub digest: Option<String>,`

"the manifest digest **when reported**" — `None` is a real state, not a
missing fixture.

## `pub(crate) struct FakeContainer {`

One container the fake holds.

## `pub(crate) struct FakeContainer` › `pub requested_image_id: String,`

The id `create` was **asked** for.

## `pub(crate) struct FakeContainer` › `pub reported_image_id: String,`

The id the runtime **reports**. A separate field, deliberately: see the
module docs.

## `struct State` › `images: BTreeMap<String, FakeImage>,`

(1) The image table, keyed by **immutable id**.

## `struct State` › `tags: BTreeMap<String, String>,`

(2) The **mutable** tag table: reference -> image id.

## `struct State` › `volumes: BTreeSet<String>,`

(4) Volume presence.

## `struct State` › `unreachable: BTreeSet<RuntimeOp>,`

(5) The availability toggle — per operation, because a runtime that
answers `ps` and fails `inspect` is a real state (see
`super::runtime`'s module docs).

## `struct State` › `failing: BTreeSet<RuntimeOp>,`

An operation that reaches the runtime and fails, which is a different
answer from unreachable and drives the other half of the refusal split.

## `struct State` › `diagnostics: BTreeMap<RuntimeOp, String>,`

A verbatim `docker` stderr this operation answers with, classified by
the production classifier rather than by the test.

`PR6-RECOV-005`: every other arming here mints a `RuntimeError` variant
**directly**, so the whole suite could pass while the function that
decides which variant a real diagnostic becomes was wrong. This is the
one arming that goes through `super::classify_docker_failure`.

## `struct State` › `containers: BTreeMap<String, FakeContainer>,`

(6) Containers, with their owner labels and incarnations.

## `struct State` › `substitutions: BTreeMap<String, String>,`

(3) Substitution injection: what `create` will report for this name.

## `pub(crate) struct Journaled {`

---------------------------------------------------------------------------
(7) The daemon every coordinator process of a test reaches (PR11 phase 5)
---------------------------------------------------------------------------

## `pub(crate) struct Journaled {`

One runtime operation as the fake received it: which handle asked (`actor`),
the operation, its target and, for a create, the credential volumes it mounts
(`detail`). The journal is the single order of every operation of every
process that shares this fake — the order a Docker daemon imposes on its
clients, and the only order in which "observed terminated before its first
use" is a statement at all. An entry is made on entry, before an armed failure
answers, so an operation that failed has its place too.

## `pub(crate) struct Launch<'a> {`

What a start policy sees of a container being started: its labels, which name
the invocation it runs.

## `pub(crate) enum Start {`

What a started container does. `Hold` is the fake as it always was: running
until a test moves it. `Exit` exits at once with the given execution — a probe
that succeeds. `Run` runs the given body on a thread of its own as the
container's process: the container is running until the body returns, and a
stop cancels the body through the `Cancellation` it was handed and waits for
it. `engine::topology::scaffold::Contained` makes the body the scaffold
double's invocation of the same request, so a coordinator's containers answer
to the deterministic scheduler (the PR11 record, R-AP).

## `pub(crate) type StartPolicy = Arc<dyn Fn(&Launch<'_>) -> Start + Send + Sync>;`

Chosen per handle (`starting`), so processes sharing one fake start their
containers differently: a live coordinator's run the double, a dead one's hold
until its process is killed, a probe exits.

## `struct Starting(Option<StartPolicy>);`

A named wrapper so that `FakeRuntime` keeps its `Debug`: a policy is a closure.

## `struct Process {`

A `Start::Run` container's process: its kill switch and its thread.

## `struct Pace {`

A barrier on one container: each of `ops` on `target` proceeds only once
`parties` different actors have arrived at it. Two reclaimers paced at `Stop`
and `Remove` have both listed and classified the container before either kills
it, and have both observed it terminated before either removes it — the
interleaving "two concurrent reclaimers converge" is about, forced rather than
hoped for, and deterministic without a clock. A party that never arrives fails
the one waiting after `PACE_BOUND`, and the message counts both.

## `pub(crate) const PACE_BOUND: Duration = Duration::from_secs(120);`

How long a paced party waits for the other: a bound that decides a failure,
never an order.

## `struct State` › `journal: Vec<Journaled>,`

(7) The journal.

## `struct State` › `processes: BTreeMap<String, Process>,`

(7) The processes of `Start::Run` containers that are running, by container
name.

## `struct State` › `pace: Option<Pace>,`

(7) The barrier a test armed, if any.

## `impl Drop for State {`

The last handle's drop cancels every process still running and joins it, so a
test that fails with containers held leaves no thread behind. A process body
never touches the fake's state, so the join cannot wait on the lock being
dropped.

## `pub(crate) struct FakeRuntime {`

`Clone`, with its state behind an `Arc<Mutex<_>>` since the repair round of
`916852c9`: a Runner owns its runtime as a `Box<dyn ContainerRuntime>`, and
the test that hands the production `ContainerRunner` a fake must still be
able to arm the fake mid-test and inspect what survived afterwards. The
shared ownership is the test's handle beside the runner's, nothing more;
the trace was already shared the same way.

The fake container runtime.

Interior-mutable so it can be handed out as `&dyn ContainerRuntime` and
still be armed and inspected by the test that holds it.

Three fields since PR11 phase 5, all per handle except the condition: `changed`
is the condition every journal entry and every process exit signals (shared
by every handle); `actor` is the handle's name in the journal; `starting` is
its start policy. A handle made by `new` is unnamed and holds every container
it starts, as the fake always did.

## `impl FakeRuntime` › `pub(crate) fn new(trace: ContainerTrace) -> Self {`

A runtime holding nothing, recording into `trace`.

## `impl FakeRuntime` › `fn enter(&self, op: RuntimeOp, target: &str) -> Result<(), RuntimeError> {`

Record the call and answer whether the operation may proceed.

## `impl FakeRuntime` › `pub(crate) fn add_image(&self, id: &str, digest: Option<&str>) {`

-- (1) the image table, keyed by immutable id --------------------------

## `impl FakeRuntime` › `pub(crate) fn add_image(&self, id: &str, digest: Option<&str>) {`

Put an image in the table under its **id**, with its digest or without
one.

## `impl FakeRuntime` › `pub(crate) fn tag(&self, reference: &str, id: &str) {`

-- (2) the mutable tag table -------------------------------------------

## `impl FakeRuntime` › `pub(crate) fn tag(&self, reference: &str, id: &str) {`

Point a reference at an image id.

## `impl FakeRuntime` › `pub(crate) fn move_tag(&self, reference: &str, new_id: &str) {`

**Move** an existing reference to another id, leaving the old id in the
table.

ST-20: "a resume after the recorded reference was moved to another image
warns and creates every container from the recorded id". The old id must
stay resolvable or that sentence has no fixture.

## `impl FakeRuntime` › `pub(crate) fn substitute_reported_image_id(&self, name: &str, reported: &str) {`

-- (3) reported image ids with substitution injection ------------------

## `impl FakeRuntime` › `pub(crate) fn substitute_reported_image_id(&self, name: &str, reported: &str) {`

Arm `create` to report `reported` for the container called `name`,
whatever id it is asked to create from.

## `impl FakeRuntime` › `pub(crate) fn add_volume(&self, name: &str) {`

-- (4) volume presence toggles -----------------------------------------

## `impl FakeRuntime` › `pub(crate) fn add_volume(&self, name: &str) {`

Make a volume present.

## `impl FakeRuntime` › `pub(crate) fn remove_volume(&self, name: &str) {`

Make a volume absent.

## `impl FakeRuntime` › `pub(crate) fn set_unreachable(&self, op: RuntimeOp) {`

-- (5) the availability toggle -----------------------------------------

## `impl FakeRuntime` › `pub(crate) fn set_unreachable(&self, op: RuntimeOp) {`

Arm one operation unreachable.

## `impl FakeRuntime` › `pub(crate) fn set_all_unreachable(&self) {`

Arm every operation unreachable — the whole daemon being down.

## `impl FakeRuntime` › `pub(crate) fn set_reachable(&self, op: RuntimeOp) {`

Make one operation reachable again.

## `impl FakeRuntime` › `pub(crate) fn set_docker_stderr(&self, op: RuntimeOp, detail: &str) {`

Arm one operation to answer with a **verbatim `docker` stderr**, whose
classification is the production classifier's rather than the test's.

## `impl FakeRuntime` › `pub(crate) fn set_failing(&self, op: RuntimeOp) {`

Arm one operation to reach the runtime and fail.

## `impl FakeRuntime` › `pub(crate) fn seed_container(`

-- (6) owner labels, incarnations ---------------------------------------

## `impl FakeRuntime` › `pub(crate) fn seed_container(`

Put a container in the table.

`requested_image_id` and `reported_image_id` are **two arguments** and
never one: see the module docs.

## `impl FakeRuntime` › `pub(crate) fn set_container_state(&self, name: &str, state: Liveness) {`

Move a container to another liveness state.

## `impl FakeRuntime` › `pub(crate) fn set_execution(&self, name: &str, execution: ContainerExecution) {`

Give a container an exit status and output.

## `impl FakeRuntime` › `pub(crate) fn container(&self, name: &str) -> Option<FakeContainer> {`

What the fake holds for `name`, if anything.

## `impl FakeRuntime` › `pub(crate) fn container_names(&self) -> Vec<String> {`

Every container name the fake holds, sorted.

## `impl FakeRuntime` › `pub(crate) fn calls(&self) -> Vec<RuntimeOp> {`

The ordered call log — every operation this runtime was asked to
perform.

Most of this slice's obligations are orderings, and "a suite that proves
the *set* of operations happened without pinning their order holds none
of them". The log shares its handle with the funnel's trace, so a single
sequence contains the funnel phases, the durability steps and these.

## `impl FakeRuntime` › `pub(crate) fn acting_as(&self, actor: &str) -> Self {`

Another handle on the same fake, named `actor` in the journal, holding every
container it starts until told otherwise. Each process of a test gets its own —
each child through its daemon, and the parent's coordinator, pre-flight and
census — so the one order also says who did what.

## `impl FakeRuntime` › `pub(crate) fn starting(mut self, policy: StartPolicy) -> Self {`

This handle's start policy.

## `impl FakeRuntime` › `pub(crate) fn journal(&self) -> Vec<Journaled> {`

The journal so far.

## `impl FakeRuntime` › `pub(crate) fn await_journal(`

Wait until `done` holds of the journal, or `within` passes, and say whether it
held. Every entry and every process exit wakes it: this is how a parent learns
that a child coordinator has its containers running without asking the child
anything.

## `impl FakeRuntime` › `pub(crate) fn pace(&self, target: &str, ops: &[RuntimeOp], parties: usize) {`

Arm the barrier; see `Pace`.

## `impl FakeRuntime` › `fn paced(&self, op: RuntimeOp, target: &str) -> Result<(), RuntimeError> {`

The barrier's wait, made on entry.

## `impl FakeRuntime` › `fn enter_with(&self, op: RuntimeOp, target: &str, detail: String) -> Result<(), RuntimeError> {`

`enter` with a create's volumes: the journal entry, then the barrier, then the
armed answers as before.

## `impl FakeRuntime` › `fn daemon_create(&self, spec: &CreateSpec) -> Result<CreatedContainer, RuntimeError> {`

The fake's own create, which the trait method delegates to and the daemon
calls.

**Why the four effect bodies moved out of the trait methods.** `clippy.toml`
denies calling `ContainerRuntime::create`, `start`, `stop` and `remove` outside
the container funnel's allowlisted modules, and this file forbids the lint.
The daemon performs a child's create by calling the fake's own body — another
item — so a child's operation reaches the same code a local caller's does, and
no file calls a governed method it may not.

## `impl FakeRuntime` › `fn daemon_start(&self, name: &str) -> Result<(), RuntimeError> {`

Start, then do what the handle's policy says the container does.

## `impl FakeRuntime` › `fn settle_process(&self, name: &str) {`

At every observation: a `Start::Run` container whose process has returned is
`Exited`, with the process's output as its execution.

## `impl FakeRuntime` › `fn kill_process(&self, name: &str) {`

Cancel a `Start::Run` container's process and wait for it.

## `impl FakeRuntime` › `fn exited(&self, name: &str, process: Process) {`

Join a process and record its execution; a body that panicked exits with no
code.

## `fn daemon_create(&self, spec: &CreateSpec) -> Result<Creat…` › `let reported = state`

The reported id is a **separate input** from the requested one. With
nothing injected the healthy runtime reports what it was asked for;
with an injection it reports something else, and that is the only
reason `substituted_image_id_refused_before_start` is constructible.

## `fn daemon_stop(&self, name: &str, _mode: StopMode) -> Result<Settled, RuntimeError> {`

The armed answer goes through the production normalizer
(`super::settle_stop`), so a diagnostic set with `set_docker_stderr` is
classified by the same function that classifies the daemon's: an
already-stopped or absent answer is `ProcessGone` and moves the fake's
container to `Exited`; a removal-in-progress answer is
`RemovalInProgress` and moves nothing, as the daemon's flag moves nothing.
A fake that minted the typed answer itself would test its own table.
Idempotent and tolerant of already-gone as before: a container the fake
does not hold is a stop that succeeded, because two concurrent reclaimers
must converge.

A `Start::Run` container's process is cancelled and waited for once the stop
is entered and before its answer is settled, so a stop that succeeded leaves
no process running — what `docker kill` does to a container's process.

## `fn daemon_remove(&self, name: &str) -> Result<Settled, RuntimeError> {`

The same through `super::settle_remove`: the container leaves the fake only
on `ProcessGone`, so a test that arms another reclaimer's removal sees the
container survive exactly as the daemon's would.


## `pub(crate) const RUNTIME_REQUEST: &str = "UPSTROKE-RUNTIME-REQUEST ";`

The wire between a child process and the parent's daemon: a child writes
`RUNTIME_REQUEST` and one JSON object naming `op` and its arguments on one
stdout line; the parent answers on the child's stdin with `RUNTIME_REPLY` and
`{"ok": …}` or `{"err": {kind, operation, detail}}` on one line; `CHILD_EVENT`
and a JSON value is a child's report to its test. A reader finds a prefix
anywhere in a line, because libtest's own `test <name> ... ` shares the child's
stdout without a newline.

## `impl FakeRuntime` › `pub(crate) fn serve(`

The daemon loop: every request a child sends is answered against this handle,
in the order it arrives, until the child's stdout closes; its events go to
`events`, and anything else (libtest's lines) is returned for diagnosis. One
daemon thread per child and one fake for all of them, so a child's operations
interleave with the parent's and every other child's in the journal.

## `impl FakeRuntime` › `fn answer(&self, request: &Value) -> Value {`

One request, answered. The read-only operations go through the trait, whose
read-only methods are not governed; the four effects through the `daemon_*`
bodies.

## `pub(crate) struct LinkedRuntime {`

A child process's container runtime: every operation is a request line to the
parent's daemon and a wait for its reply. One request at a time (`turn`), so
replies cannot cross; a child coordinator's pipeline threads wait their turn as
they would on a daemon's socket. A parent that does not answer within
`LINK_BOUND` is `Unreachable`, which the census and the runner already refuse
over.

## `impl LinkedRuntime` › `fn call(&self, op: RuntimeOp, arguments: Value) -> Result<Value, RuntimeError> {`

Send, wait, decode.

## `fn spec_json(spec: &CreateSpec) -> Value {`

The codec: each type the trait carries, as JSON and back (`*_json`, `*_of`). A
path travels as its lossy string, which every path a test makes is.

## `pub(crate) struct FakeOwnerLiveness {`

---------------------------------------------------------------------------
(6b) Liveness simulation
---------------------------------------------------------------------------

## `pub(crate) struct FakeOwnerLiveness {`

The fake owner-liveness probe.

`determinism` lists "liveness simulation" among the fake container runtime's
capabilities; in this tree it is a **separate seam**, because liveness is a
non-blocking probe of another run's `run.lock` on this host
(`crash_reconstruction`: "probe that run's run.lock non-blocking (is_running
semantics: src/rundir.rs:619-652)") and not a question the container runtime
could answer. Splitting it is also what makes "the coordinator incarnation
id … is **never read from lock-file contents**" structurally true: the
answer is one bit and carries no incarnation to read.

## `impl FakeOwnerLiveness` › `pub(crate) fn new() -> Self {`

Nobody is alive.

## `impl FakeOwnerLiveness` › `pub(crate) fn set_live(&self, public_run_dir: &Path) {`

Say that a coordinator holds this public run directory's lock.

## `impl FakeOwnerLiveness` › `pub(crate) fn set_dead(&self, public_run_dir: &Path) {`

Say that it does not.

## `pub(crate) struct RecordingHooks {`

---------------------------------------------------------------------------
A recording observer
---------------------------------------------------------------------------

## `pub(crate) struct RecordingHooks {`

Hooks that record into a trace and can be armed to fail at one site phase.

The fault arm is the error-return mode at a hook phase, which is what proves
a partial sequence converges — an `Err` from `After` is returned *after* the
primitive ran.

## `impl RecordingHooks` › `pub(crate) fn new(trace: ContainerTrace) -> Self {`

An observer sharing `trace` with the runtime.

## `impl RecordingHooks` › `pub(crate) fn fail_at(&mut self, site: EffectSiteId, phase: HookPhase) {`

Make the funnel return `Err` at one phase of one site.

## `pub(crate) const REQUIRE_DOCKER: &str = "UPSTROKE_REQUIRE_DOCKER";`

---------------------------------------------------------------------------
The Docker gate — loud and counted, never silent
---------------------------------------------------------------------------

## `pub(crate) const REQUIRE_DOCKER: &str = "UPSTROKE_REQUIRE_DOCKER";`

The environment variable that turns a skip into a failure.

A gated suite that skips silently everywhere is the "green because the test
could not run" failure this project keeps paying for. On a machine with
Docker the suite is run **with this set**, so a skip is a red test; CI and
the Windows guest have no container runtime and run without it.

## `pub(crate) const DOCKER_GATED_TESTS: &[&str] = &[`

Every Docker-gated test in this slice, by name.

Written out rather than counted, because *which* gated test disappeared is
the finding. `runner::container::tests::every_docker_gated_test_is_named_and_present`
asserts each name is a `fn` in the container sources, so a gated test that is
deleted or renamed fails rather than silently leaving the list shorter.

**Lanes A, B and C: append your gated test's name here.**

## `"real_docker_kill_on_an_already_exited_container_is_tolerated",`

Repair round F1: the three defects real Docker found in this file.

## `"real_docker_runs_from_the_recorded_image_id_and_composes_over_the_image_environment",`

Lane A: the ContainerRunner against the real runtime.

## `"real_docker_census_reclaims_a_dead_owner_and_spares_a_live_one",`

Lane C: the startup census against the real runtime.

## `"real_docker_a_gate_write_outside_every_declared_mount_fails",`

Repair round R1: what "confines gate-executed repository code" and
"pre-flight certifies the environment that will actually spend" mean
against a real daemon rather than against a spec.

## `"real_docker_renders_a_comma_bearing_label_value_whole",`

Repair round R2: the two tables whose oracle has to be the daemon.

## `"real_docker_creates_an_absent_named_volume_rather_than_refusing",`

Repair round R3b: the two daemon behaviours the engine has to compensate
for, and the two claims whose only honest oracle is a live container.

## `pub(crate) fn absent_reason() -> String {`

Why a gated test skipped.

A value rather than a comment, so a skipping test *reads* the reason and a
skip that had stopped saying anything is a compile-time unused value rather
than a silence.

## `pub(crate) fn docker_gate(test: &str, trace: ContainerTrace) -> Result<Box<DockerCli>, String> {`

Ask whether the Docker-gated half of this suite can run.

`Ok` is the CLI bound to the trace; `Err` is the reason, which the caller
must assert on before returning — that is what makes the skip loud in the
test body rather than an unremarked early `return`.

### Panics

When `test` is not in [`DOCKER_GATED_TESTS`], so an uncounted gated test
cannot exist; and when [`REQUIRE_DOCKER`] is set and Docker is not
available, which is how a skip becomes a **failure** on a machine that has a
runtime.

## `pub(crate) fn slot_repo_key() -> &'static str {`

The repository key every container name **that a pre-clean touches** is
built from, **scoped to the build slot this process is running in**.

**Not "every container name in a Docker-gated test".** That is what this
sentence said after the move from `exec.rs`, where the narrower "every
container name in this module" was true; `runner::container::tests` names
Docker-gated containers with bare literals carrying no repo key at all and
reclaims them with `let _ = docker.remove(name);` rather than through
[`preclean_names`]. Those names are outside this rule and outside the guard
that enforces it. `R5-SEAMS-003`.

**And the guard is inert when there is no slot.** With `CARGO_TARGET_DIR`
unset, [`scoped_repo_key`] returns the fixed `0123456789abcdef`, so two
concurrent bare `cargo test` runs on one box derive the same key, pass
[`unscoped_names`], and each pre-clean kills the other's live container —
the state the guard exists to refuse. It is inert in exactly the
configuration with no other protection. Recorded rather than repaired,
because the alternative — a per-process key — makes the pre-clean useless,
which is the trade the "why this is not a constant" section below is about.
On this box every gate command goes through `upstroke-build`, and CI's bare
`cargo test` runs one job per machine. `R5-SEAMS-006`.

### Why this is not a constant

[`preclean_names`] kills and removes a container by name before creating it,
because no in-process cleanup runs when a process is SIGKILLed and the name a
killed run left behind is exactly the name the next `docker create` asks for.
That is correct and it is why the helper exists.

With a **fixed** key it is also hostile: two suite runs share every name, so
the second run's pre-clean kills the first run's **live** container.
`PR7-R3-CONTRACT-001`. This box runs concurrent suites by design — the whole
point of `upstroke-build`'s slot pool — so a fixed name is not a theoretical
collision, it is the normal case.

**Scoped to the slot rather than to the process** deliberately. A PID would
make every run's names unique and thereby make the pre-clean useless — it
could never match the killed run's leftovers, which is the one thing it is
for. `CARGO_TARGET_DIR` is stable across runs *in* a slot and distinct
*between* slots, so a slot reclaims its own residue and touches nobody
else's. That is the same discriminator the slot pool already uses.

**Here rather than in one caller's test module.** `b44040a` put this in
`exec.rs`'s and left `census/tests.rs` — the other of
[`preclean_names`]'s two callers — on a fixed `"cccccccccccccccc"`, so the
class stayed live on that path. A rule with one implementation per caller is
a rule each caller can be missing. It sits beside the helper it is a
precondition of, and [`unscoped_names`] makes it one the helper checks.

## `pub(crate) fn scoped_repo_key(scope: &str) -> String {`

[`slot_repo_key`]'s derivation, as a pure function of the scope.

Separated from the `LazyLock` so it is testable: the cache is computed once
per process, so a test that set the variable and called [`slot_repo_key`]
would assert whatever the first caller in that process happened to see.

Sixteen hex characters, because `workspace_manager`'s `REPO_KEY_HEX_CHARS`
says a repo key is.

## `pub(crate) fn scoped_repo_key(scope: &str) -> String` › `return "0123456789abcdef".to_owned();`

No slot -- a bare `cargo test`, where nothing else is running.

## `pub(crate) fn unscoped_names(names: &[&super::intent::ContainerName]) -> Vec<String> {`

The names among `names` that a **concurrent run could also ask for**, as the
rendered strings, so a refusal can say what it refused.

A name whose repo-key component is not this slot's is a name another slot's
suite builds identically, and [`preclean_names`] kills by name with no
liveness check. Checking the *component* rather than the whole name is
deliberately narrower than the property ("no two concurrent runs ask for the
same name"): a caller that scoped some other component instead gets a loud
refusal it can widen this rule for, rather than a silent stranger-kill.

A name that will not parse is reported too. The pre-clean is the one place
that acts on a name it did not build, and an unparseable one is a name whose
scope cannot be established at all.

## `pub(crate) fn preclean_names(`

Reclaim the container names a gated test is about to create, before it
creates them.

`reviews/FINDINGS.md` §16. Two gated tests went red on a head whose only
change was a documentation edit, and passed in isolation minutes later:
four containers from an earlier run this session had been **SIGKILLed** when
the box exhausted its inodes — two `Exited (137)`, two still `Created` —
and their names were the deterministic ones those tests recreate, so
`docker create` answered `Conflict. The container name … is already in use`.

Both tests already clean up after themselves, one with a closure on every
exit path and one with a `LeaveNoResidue` guard. Both are correct and
neither can help: **no in-process cleanup runs when the process is
SIGKILLed.** The only cleanup that survives is one the *next* run performs.

The idiom is correct here **only because the names are deterministic**:

> A pre-clean removes the previous run's residue exactly when the name
> recurs. Keyed by something unique per process — a pid, a ULID — it can
> never name anything an earlier run created, and it degrades into an
> unconditional retry that cleans nothing.

So this takes the exact names the caller is about to use, and callers must
build them from their own fixed constants. A caller that passes a pid-keyed
or ULID-keyed name gets a no-op that looks like protection.

This goes through [`super::reclaim`] rather than calling `stop`/`remove`
directly. That is not ceremony: `fake.rs` **re-denies** the effect lints at
its own module level (`PR6-LANEF-004` — a lint level is scoped by the module
tree, so every out-of-line child of `runner::container` had been silently
inheriting the funnel's allow), so a raw primitive here does not compile.
The funnel is also the right answer on its merits: it is the packet's own
reclaim order, every step idempotent and tolerant of already-gone.

`view_path` is `None` because a previous run's Git view lives under **its**
scratch root, which was keyed by that run's pid and is unreachable from
this one. The container name is the only part of the residue that is global
to the daemon, and it is the only part that can collide.

### Panics

When a name cannot be reclaimed for any reason other than its absence —
a pre-clean that fails quietly leaves the conflict it exists to prevent,
and the test then fails somewhere far less informative.

## `let unscoped = unscoped_names(names);`

The precondition, checked rather than documented. Before this the doc
below said "callers must build them from their own fixed constants" and
one of the two callers did exactly that -- fixed, and therefore shared
with every concurrent slot. See [`unscoped_names`].

## `struct State` › `reaper_program: Option<PathBuf>,`

The relay stub once `install_reaper_relay` installed one; `None` until then. Shared by every
`acting_as` view of one fake, so a runner built over any of them arms its reaper with the same
program.

## `struct State` › `covers: Option<Vec<(String, bool)>>,`

`Some` once `observing_covers` asked for it: every container started from then on, with
whether an armed container reaper in this process selected its labels at the instant it
started.

## `impl FakeRuntime` › `pub(crate) fn observing_covers(&self) {`

The design property's in-process observation (PR11 follow-up A): "while any container this
incarnation started may still be running, an armed reaper holding the run's container scope
exists". `daemon_start` records, for each start, whether
`agent::proc::armed_container_reaper_selects` answers yes for the container's
`upstroke.private_root` and `upstroke.incarnation` labels. It sees only reapers armed in this
process, so the two-process witnesses do not use it; they observe the reaper's calls instead.

## `impl FakeRuntime` › `pub(crate) fn starts_observed(&self) -> Vec<(String, bool)> {`

What `observing_covers` recorded, in start order.

## `pub(crate) const NO_OP_REAPER_PROGRAM: &str = "/usr/bin/true";`

The fake's reaper program until a relay is installed: a reaper armed over it that outlives its
coordinator runs `/usr/bin/true ps …`, lists nothing and exits, so the many tests that build a
`ContainerRunner` over the fake and never look at a reaper arm one that touches nothing.
`/usr/bin/true` exists on both Unix legs; Windows arms no reaper.

## `impl ContainerRuntime for FakeRuntime` › `fn reaper_program(&self) -> PathBuf {`

The relay stub if one is installed, else `NO_OP_REAPER_PROGRAM` — never the trait's `docker`
default, which would arm a reaper over a real CLI where one is installed.

## `impl FakeRuntime` › `fn observe_cover(&self, name: &str, labels: &BTreeMap<String, String>) {`

Called from `daemon_start` after the container is marked running and before its start policy
plays it. On Windows there is no reaper and nothing is observed as covered.

## `fn reaper_stub() -> String {`

The relay: a `/bin/sh` program a reaper execs as its `docker`. **It finds its relay from its
own path** (`relay=${0%/*}`; the reaper `execv`s the absolute program, so `$0` is it), never
from an environment variable — round R5's stub read a variable only the two-process witness
set, so the in-process control's reaper wrote its calls nowhere the test looked (`R6-D2`).
It appends each call's arguments to `calls`, tab-separated. `ps` answers from `listing` the
way the daemon would: each line is a container's name, private-root label and incarnation
label, and only the names whose two labels equal the two `--filter label=…` values are
printed — so a scope that does not select a container does not find it. `rm` drops the named
container from `listing`; `kill` changes nothing, as `docker kill` leaves a container listed
until it is removed.

## `impl FakeRuntime` › `pub(crate) fn install_reaper_relay(&self, relay: &Path) -> PathBuf {`

Creates the relay directory with an empty listing, writes the stub through
`write_program_in_its_own_process` — a process of its own, so no fork of this multithreaded
test process can inherit a writer of a file that is about to be exec'd (`R6-D3`) — and makes
it this fake's reaper program.

## `impl FakeRuntime` › `pub(crate) fn publish_for_reaper(&self, relay: &Path) -> Vec<(String, String, String)> {`

Writes the listing from every container the fake holds, with their recorded labels (`-` for
one a seeded container lacks). A witness publishes before it kills the coordinator: the reaper
lists within ten milliseconds of the death.

## `impl FakeRuntime` › `pub(crate) fn reaper_calls(relay: &Path) -> Vec<Vec<String>> {`

The calls the stub recorded, one argument vector each, in order.

## `impl FakeRuntime` › `pub(crate) fn deliver_reaper_calls(&self, relay: &Path) -> Vec<String> {`

Applies each recorded `kill` and `rm` to this fake as the actor `reaper`, so a witness reads in
the journal that the containers were stopped and removed by the reaper's own calls.

## `const REAPER_PROGRAM_QUERY: &str = "reaper-program";`

A request on the link that is not a runtime operation: the child's `LinkedRuntime` asks the
parent's fake which program its reaper runs. The parent's fake is the one source, so a
two-process witness installs its relay once, on the fake it serves, and the child coordinator
arms over that relay without being told anything else.

## `impl LinkedRuntime` › `fn exchange(&self, op: RuntimeOp, request: &Value) -> Result<Value, RuntimeError> {`

One request and its reply, under the runtime's turn; `call` builds a runtime operation's
request and hands it here, and `reaper_program` sends the query.

## `impl ContainerRuntime for LinkedRuntime` › `fn reaper_program(&self) -> PathBuf {`

The parent fake's answer. A link that does not answer yields an empty path, which the reaper's
program resolution refuses, so the launch is refused rather than arming a reaper that would
silently reclaim nothing.

