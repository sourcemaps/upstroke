# `src/engine/topology/coordinator/tests/runners.rs`

Extended notes for [`src/engine/topology/coordinator/tests/runners.rs`](../../../../../../src/engine/topology/coordinator/tests/runners.rs).

The code is the authority for what it does; this file is the whole of its prose but for the
concurrency protocol standards §10 places at its site. Each section
here is headed by the line of code it describes, spelled as it is in the source, so the heading is
the grep string that finds the code.

## `fn every_container_invocation_is_launched_and_released_on_its_own_at_width_three() {`

R19 and R26 under concurrency (phase 5, R-AP). A width-3 run whose pipelines run the production
`ContainerRunner` over the shared fake, each container's process being the scaffold double's
invocation (`scaffold::Contained`), so the scheduler holds and releases containers as it holds and
releases invocations. At every quiescent point the physical inventory — the containers running in
the runtime, the intents under `<R>/containers`, the views under `<R>/views` — is exactly the names
of the invocations inside the runner; at the end all three are empty. Every process the double ran
had one container, created once and removed once, and each mounts its agent's credential volume
exactly when its role is given one. Three containers ran at once.

## `fn holds_nothing(run_dir: &std::path::Path, repo: &std::path::Path) -> Result<(), String> {`

R17, observed (R-AQ): nobody holds the run — `rundir::is_running` answers from this process's
claims, then from the OS — and the fixture checkout's worktree lock can be taken, and is let go at once.

## `fn served(`

A child coordinator (`container_coordinator_child`) served the parent's fake runtime
(`scaffold::Served`), its standard error kept as `<name>.stderr` in the test's `logs` directory — which is
also its temporary directory (`scaffold::child_temporary_of`). So a fresh child's fixture, which the
parent adopts once the child is dead, and the neutral Git configuration the child writes lie inside the
`logs` guard, which each test binds before its children and the runs it adopts from them, so they are
dropped first and the guard then reclaims the rest; the tests keep that directory's tag
short (`census-b`, `resume-f`, `crashes-g`, `converge-h`, `reapers`), since a fresh child's paths nest
inside it on Windows too (the record's phase-7 path arithmetic).

## `fn a_foreign_census_reclaims_a_dead_coordinators_containers_and_leaves_a_live_coordinators_running()`

ST-16 (b) under concurrency, in three processes. A dead owner — a child holding its run lock and two
containers, killed — leaves its containers running in the shared runtime and its lock free. The live
coordinator A, this process, runs three pipelines' containers; at its first quiescent point with all
three inside the runner, a foreign write command in a second repository under the same private root
(R-AN) runs its census as a child, through the parent's daemon. The census reclaims exactly the dead
owner's containers (`foreign-run-dead-owner`) and classifies A's as a live owner's
(`foreign-run-live-owner`), probing A's `run.lock` across the process boundary; A's three run on
across it, and every removal of them in the journal is A's own. The foreign command's first
invocation mounts the shared credential volume, and the journal puts it after the census observed
every dead container terminated and removed it. R17: the owner's lock went with its process, A's is
held until A ends and then nobody's, and the foreign command holds nothing after it exits; its
pre-lock refusal left nothing held (it took the worktree lock right after, in the same process). The
owner's release is read as a bounded wait (`within`, under `BOUND`), as `holds_nothing` reads the
others: `rundir::is_running` reads the run's cleanup lease once the run lock is free, and a sibling test
thread's fork can hold a copy of that lease for a moment after its last holder ends (the mechanism of
`PR281-CLEANUP-LEASE-HOLD-OUTLIVED-AND-ITS-UNREADABLE-TWIN`). A hold that outlives the bound fails the
test.

## `struct LedgerWatch<'p> {`

A container observer for the census of a resume, recording the pre-flight's ledger — its
settlements and its running registrations — at every container site the census passes. With
`CensusWatch` it is the recovery order's hooks.

## `fn a_resuming_incarnation_reclaims_its_earlier_incarnations_containers_before_its_ledgers_probes_and_admission()`

ST-16 (f) and R-AO. Incarnation 1, a fresh child, is killed with three pipelines' containers
running; incarnation 2, a resuming child, reclaims those three in its census and is killed while its
own shell probe's container runs. Incarnation 3, this process, resumes: its census reclaims
incarnation 2's probe — the same deterministic `InvocationId` as its own shell probe, under another
container name and intent path — and the journal puts every census operation on it before
incarnation 3's first container operation. At every reclaim site the census passed, the pre-flight's
ledger held nothing; the probe ran after it and settled; the broker was empty when it was built.
The three containers incarnation 3 starts afterwards at width three are a live owner's to a foreign
census run as a child at its first quiescent point with three inside the runner, which reclaims
nothing and leaves them running. Each dead incarnation's holds went with its process.

## `fn crashes_across_three_incarnations_with_pipelines_in_flight_leave_every_orphan_reclaimed_and_no_name_twice()`

ST-16 (g). Incarnation 1 is killed with three pipelines in flight; incarnation 2 resumes as a child,
reclaims incarnation 1's three containers, settles its three attempts interrupted, passes its
pre-flight (probes exit) and is killed with three pipelines of its own in flight; incarnation 3, this
process, reclaims those three, settles their attempts and completes. Across the three incarnations no
container name and no intent path occurs twice, and wherever two incarnations ran one
`InvocationId` — the shell probe, in incarnations 2 and 3 — their names differ. Six
`attempt_interrupted` and two `run_resumed` in the log; nothing left under the private root.

## `fn a_foreign_census_and_a_resuming_incarnation_converge_on_one_dead_container_as_two_processes()`

ST-16 (h), as two processes. A dead owner's two containers are under the private root; the fake paces
the first at `Stop` and `Remove` for two parties, so the resuming incarnation (this process's
recovery order) and a foreign write command (a child in a second repository) have both classified it
before either kills it, and both observed it terminated before either removes it. Both reclaim it —
each actor's `Stop` and `Remove` are in the journal — neither refuses, and the root converges clean.

## `fn a_surviving_reaper_hold_refuses_the_next_coordinator_until_released_and_is_never_reset_at_width_three()`

R28 at width three (Unix; R-AQ, R-Z). A width-3 run is shut down with its three workers in flight;
each pipeline's Runner call carried exactly the run's cleanup lease path (`CarriedLeases` records the
calls). A simulated surviving reaper — `rundir::tests::cleanup_hold_child`, a real process holding
the shared lock — then holds R28: the next coordinator's recovery order is refused at its lock
acquisition, naming the hold, and the hold is exactly where it was; once the reaper lets go, the
resume proceeds with empty ledgers and completes.

## `fn a_killed_coordinators_reapers_settle_its_pipelines_processes_under_r28_before_the_next_one_resumes_at_width_three()`

The OS matrix's Unix row at the coordinator, with real processes (R-AQ, R-AR). A coordinator child's
three pipelines each run a real host process through the production `HostRunner` (`HostHeld`: a
shell that records its pid and sleeps). The shell is handed the pid file's path as its argument, never
inside its program text (standards §9), and the pid directory's own name carries an apostrophe and a
space, so a path a shell would have to quote is exercised on every run: at `a9e88039`, which spliced
the path into the program in single quotes, no pid was recorded and the test failed at its bound
(round R5, the full review's `FULL-REG-1`). While they run, the run's cleanup lease is held — by reapers
spawned from pipeline threads, which entered no scope and hold it through the paths the coordinator
carried (R-Z). The coordinator is killed; every one of the three processes is gone once its reaper
settles, the holds are released after them, nothing of the dead coordinator is held, and the next
coordinator resumes with empty ledgers and completes.

## `fn f_hosted_linked_registry(sequence: &str, child: &str, completes: bool) {`

T-ENV-TOPO, the external-prune follow-up's integration correction at the topology, under the
production host runner (F), on Unix and on Windows. It builds one layout's run, with the run's
`worktrees` store a link into another repository (R-C-REGISTRY's part 2):

- **C4:** an absolute `GIT_COMMON_DIR`, the base at `<root>/repo`.
- **C5:** a relative value, the base at `EQUAL_DEPTH_BASE`, the proposal's equal-depth construction.

**The plan** is two tasks, alpha and beta (`f_hosted_tasks`), which write disjoint paths, at width
two.
- **Both start from the start head.** The coordinator's first admission dispatches both before it
  handles any pipeline's message. `admit` selects until nothing more is admitted. Its dispatches
  record state themselves, but a candidate, without which no integration can advance the head,
  enters the fold only through a message `receive_one` handles after `admit` returns. A retry keeps
  its generation's base.
- **The first merge is fast.** Whichever candidate merges first merges fast, as sequence 0.
- **The second is stale.** The other's base is then no longer the head, so it integrates stale, as
  sequence 1 (`F_HOSTED_VERIFIED`). Its staging cherry-pick runs, and so do its verification's gate and
  review, over the host runner.

**The parent:**

- **The run and the link.** It creates the run at width two, drops it and plants the link.
- **C5's value.** It derives the relative value from the canonical paths. It asserts that the value
  names the run's common directory, climbing to the scratch root and no further. It checks this from
  the base, and from each directory the run places a role in (`f_hosted_placements`), and from
  sequence 1's merge slot.
- **The scripts.** It writes three scripts to a directory it puts first on the child's `PATH`:
  - the two agents' command-line programs. Each logs its first argument and its directory. As the
    worker, it writes `src/<task>/work.txt`; alpha's first attempt leaves the slot as it was.
    As a reviewer, called with `--prompt`, it prints the scaffold's `PASSING_VERDICT`, which the child
    is handed as `UPSTROKE_TEST_F_VERDICT`. The child's adapters, `ScaffoldAdapters::new()`, answer a
    review with that verdict themselves and do not read the output;
  - `gate`. It logs, then requires `git rev-parse --git-common-dir` to be the run's canonical common
    directory and `HEAD^{tree}` to be readable.
- **The child.** It starts `child` with the value inherited: T-ENV-TOPO's `hosted_linked_registry_child`,
  or one of T-NOREQ's (`f_hosted_no_request_child`, below).
- **A refused run** (`completes` false) is the child's to assert. After it the parent asserts only that the
  store is still the link, naming the same target, and that the other repository's refs are untouched.
- **After the child,** what the completed run left, each by its contract:
  - the store is still the link, naming the same target, and the target is kept and empty. Finalization
    removed every registration the run made, below the store pinned at derivation, and neither the link
    nor its target;
  - nothing is left under `refs/upstroke/runs/`. Finalization deletes a completed run's prepared,
    candidate and candidates refs, so none is claimed to survive;
  - the run's one integration branch under `refs/heads/upstroke/` differs from `main` by exactly
    `src/alpha/work.txt` and `src/beta/work.txt`, each holding its worker's bytes;
  - the other repository's refs are untouched.

**On Windows** the run, every assertion and each role's work are the same. What differs is how the
layout is made, and why:
- **The link is a junction** (the fixture's `link_directory`, `mklink /J`). A directory symbolic link
  needs a privilege a junction does not. A junction is a name-surrogate reparse point, so std calls it
  a symbolic link, `canonicalize` follows it, and the manager pins it as it pins any reparse point.
- **The scripts are batch shims** (`f_hosted_shim`), as `npm` installs an agent's command-line program
  there. The host runner resolves one through `PATHEXT`, and `cmd` finds the gate's the same way.
  Each runs this test binary's `hosted_linked_registry_role_child`, which does what the script does.
- **The child runs through `LinkedChild`** (`f_hosted_isolated`). `run_test_isolated` is Unix's,
  since it starts its child in a process group of its own. The parent asserts the same of the child:
  that it succeeded, and that its output says `1 passed`.
- **Paths compare canonical as the manager spells them** (the fixture's `canonical`). On Windows
  `fs::canonicalize` alone adds the verbatim prefix the manager strips. On Unix the two are the same.

T-NOREQ's rows stay Unix's; this function's Unix arms are unchanged.

## `fn a_hosted_run_under_a_linked_registry_and_an_inherited_absolute_common_dir_completes() {`

T-ENV-TOPO's C4 row, on Unix and on Windows.

## `fn a_hosted_run_under_a_linked_registry_and_an_inherited_relative_common_dir_at_equal_depth_completes()`

T-ENV-TOPO's C5 row, on Unix and on Windows.

## `fn a_hosted_review_with_no_request_under_c4_is_deferred_and_the_run_completes() {`

The witness's C4 row.

## `fn a_hosted_review_with_no_request_under_c5_is_deferred_and_the_run_completes() {`

The witness's C5 row.

## `fn a_hosted_review_with_no_request_over_a_damaged_registration_is_refused() {`

The negative, under C4.

## `fn a_hosted_review_request_whose_runner_reports_nothing_is_refused() {`

The control, under C4: an admitted request whose runner reported nothing keeps the original rule.

## `fn the_broker_ledgers_balance_at_every_end_and_start_empty_at_every_next_start_at_width_three()`

ST-09's broker rows over the ends phase 4 built: Complete, Parked, Halted, BudgetExceeded, an append
error and a shutdown, each at width three. At each end nothing is registered and unsettled, pending,
held or reserved (`balanced_at_end`); at the next start the process holds nothing it did not take,
and a resumable end's broker starts empty (`broker_is_empty`), while a Complete or Halted run is
finalized and refused before any broker is built. The killed coordinator's end is the process's
death, and its next start is asserted empty in the ST-16 (f) and (g) tests and the reaper test.

## `const REAPER_BOUND: Duration = Duration::from_secs(60);`

How long a witness waits for a dead coordinator's reaper to finish its calls. The reaper notices
the death within one of its 10-millisecond polls; the bound exists so a mutation that arms nothing
fails the test instead of hanging it. It also bounds each wait `KeptForItsBackstop` makes on the
first-probe guards' backstop: its arrival at its gate, its passage, and the end of every hold of
the run's cleanup lease.

## `fn reaper_finished(`

Polls the relay until the reaper's calls are a listing, a kill and a removal per expected
container, and a final listing, or the bound passes. That is relay completion — the calls the
relay recorded the reaper making — and not the reaper's collection: the reaper is an orphan of
the killed coordinator, and no test process waits for it.

## `fn reclaimed_by_its_reaper(reclaimed: &Reclaimed<'_>) {`

One reaper reclaimed exactly the expected containers: its first and last calls are the scope's
listing argv, one `kill` and one `rm --force --volumes` per container, nothing else; delivered
to the fake, each was stopped and removed by the actor `reaper`, none is left, and no census
listed the runtime after the death.

## `enum ProbeSchedule {`

How a first-probe guard schedules its dead coordinator's backstop around the helper's return.
`Free` names no gate, so the backstop goes through its ending on its own schedule.
`HeldAtItsListing` — Linux only, because the gate's name, `agent::proc`'s `BACKSTOP_GATE`, is
re-exported for Linux test builds only — names the backstop's test gate in the resuming
coordinator child's environment and nowhere else, so the backstop that child arms holds after it
has listed the probe and before it takes its worktree's lock, until the caller lets it go, which
the caller does only after the helper has returned. That forces, every run, the schedule the three
reviews found the old helper could not survive: the helper returns, and only then does the
backstop, having listed, take the physical worktree lock. By platform:

- Linux: `the_reapers_scope_is_the_runners_identity_and_selects_its_first_probe` runs
  `HeldAtItsListing`, the forced schedule;
  `a_resuming_incarnations_pre_flight_probe_container_is_killed_by_its_reaper_when_the_coordinator_dies_inside_it`
  runs `Free`.
- macOS and every other Unix: both run `Free`; no schedule is forced there.
- Windows: neither is compiled (`cfg(unix)`).

Neither guard is skipped anywhere it compiles. Both keep every original assertion, bound and call
vector, and both assert after the helper's return that the real fixture still has the paths the
backstop opens (`its_lawful_paths_are_present`).

## `enum GateState {`

Where a held gate stands: `Unarmed` until the helper arms it, `Armed` from just before the kill,
`LetGo` once its release is written. Kept in a `Cell`, so letting go takes a shared borrow.

## `struct KeptForItsBackstop {`

The one owner of everything a dead first-probe coordinator's backstop may still open: the real
fixture — the `WideEnv` itself, whose repository holds the worktree lock and whose run directory
holds the run lock and the cleanup lease — the kill directory (the relay's program, listing and
call log; the coordinator child's stderr and temporary directory; the gate's files), the schedule
and, on Linux, the gate's path and state. The helper makes it from the `WideEnv` and the kill
directory before it spawns the coordinator child and returns it inside `DiedInItsProbe`, so no path
out of the helper or out of its caller lets the fixture go except through this owner's drop.

Before, the helper returned a clone of the private root's path and let its `WideEnv` drop at its
return, and the fixture's scratch tree removed the repository and the run directory while the
backstop forked at the coordinator's death might not yet have opened them. Since `47fad28c` that
backstop opens the existing worktree lock, and then the lease paths, only after its listing, so
under that schedule it stood down, failing closed as it should, and the guard waited for a
reclamation that could not come. Native20's Ubuntu job failed the scope guard at `the probe was
reclaimed: []`, and its macOS job the other guard's call vector (one listing, no kill or removal);
those logs do not record the backstop's failed opens, so they are consistent with this schedule
without proving it. Three reviews named the one issue: `REGULAR16-FIRST-PROBE-FIXTURE-LIFETIME`,
`PR12-REGRESSION14-BACKSTOP-PROBE-FIXTURE-LIFETIME` and `PR12-INV11-PROBE-REAPER-FIXTURE-LIFETIME`.

## `impl KeptForItsBackstop` › `fn child_environment(&self) -> Vec<(&'static str, &std::ffi::OsStr)> {`

The resuming child's role, root and incarnation, and under `HeldAtItsListing` the gate's name and
path: that child's environment only. This process's own environment is never changed.

## `impl KeptForItsBackstop` › `fn arm(&self) {`

`HeldAtItsListing` only, right before the kill. At each point whose arrival file exists, the
backstop writes one byte there and then waits, at most 120 s (the gate's own bound, in
`agent::proc`), for the point's release file. This marks the gate armed and then writes the Decided
release, the Decided arrival file and the Listed arrival file, in that order, so a failure partway
still leaves the drop a gate to let go. Decided is armed with its release already present: it
never holds, and the byte the backstop writes there is the passage `let_the_backstop_go` waits for.
Through `workspace_manager::fixture::write_file`: a topology module names no `std::fs` write.

## `impl KeptForItsBackstop` › `fn let_the_backstop_go(&self) -> Result<(), String> {`

For an armed gate, once: writes the Listed release on a thread of its own, which it joins — a
write that fails panics that thread and becomes this `Err`, never a panic on a thread already
unwinding — then waits up to `REAPER_BOUND` for the Decided arrival byte. That is gate passage:
the backstop has decided its exclusion and opens no fixture path after it. It is a byte the
backstop wrote, not its collection. A release that could not be written leaves the gate armed, so
the drop tries it again.

## `impl Drop for KeptForItsBackstop` › `fn drop(&mut self) {`

The order, on every path, is the body's: `let_the_backstop_go` (an armed gate not yet let go is
released and its passage waited for); then `no_cleanup_hold_is_left` over the run directory; then
the real fixture is dropped; then the kill directory; and only then is a failed wait reported,
said on stderr while the thread unwinds and a panic otherwise. On the positive path the guard has
already let the gate go and seen relay completion, so the drop confirms that no hold is left and
lets go. After a panic inside the helper, the coordinator's `Served` handle, declared after this
owner, drops first, killing and collecting the coordinator. On the `Free` schedule, a backstop
that has not yet taken its exclusion when a failed test lets the fixture go stands down, as it
must.

## `fn no_cleanup_hold_is_left(public: &std::path::Path) -> Result<(), String> {`

Waits up to `REAPER_BOUND` until `rundir::observe_cleanup_hold` reports no hold of the run's
cleanup lease: the backstop, which takes its shared hold once its exclusion is established and
keeps it until it exits, and every other holder have let it go. A released hold says that no lock
is held, not that any process was collected.

## `fn the_backstop_reached(arrival: &std::path::Path) -> bool {`

Whether the backstop has written its byte to a gate point's arrival file.

## `struct DiedInItsProbe {`

What the helper returns: the owner of the real fixture, the kill directory and the gate
(`KeptForItsBackstop`), the fake the coordinator child was served by, the relay and its program,
the private root, the probe's container name, and how many census listings the fake had answered
before the death.

## `impl DiedInItsProbe` › `fn its_lawful_paths_are_present(&self) {`

After the helper's return, on every schedule: the worktree lock
(`<repository>/.git/upstroke-worktree.lock`, `rundir::worktree_lock_file`), the run lock
(`rundir::lock_file`) and the run's cleanup lease are still files of the real fixture — the paths
the dead coordinator's backstop takes, asks `F_GETLK` of, and holds, after its listing.

## `impl DiedInItsProbe` › `fn held_at_its_listing(&self) {`

Linux, `HeldAtItsListing`: waits up to `REAPER_BOUND` for the backstop's arrival at its Listed
point, then asserts that the relay has recorded exactly one call, the scope's listing. The
backstop has listed the probe, killed and removed nothing, and holds where it would next take its
worktree's lock.

## `impl DiedInItsProbe` › `fn let_its_backstop_go(&self) {`

Linux, `HeldAtItsListing`: `let_the_backstop_go` on the positive path; its failure is the test's.

## `fn a_resume_killed_inside_its_pre_flight_probe(`

A durable run, then a resuming incarnation in a child over the parent's fake — the frozen
recovery order with a `RunPreflight` over a `ContainerRunner` — killed while its shell probe's
container runs. The relay is installed on the fake the child is served by, so the child's
runner arms over it with nothing passed in the environment; under `HeldAtItsListing` the child's
environment also names the backstop's gate, which is armed after the relay's listing is published
and before the kill. The child is killed and collected, a wait status this process reaps; its
backstop is not this process's child. The real fixture and the kill directory leave the helper
inside its `KeptForItsBackstop`.

## `fn a_resuming_incarnations_pre_flight_probe_container_is_killed_by_its_reaper_when_the_coordinator_dies_inside_it()`

`R6-C2`: the incarnation's first container — a pre-flight probe inside the frozen `recover.rs` —
is covered; its reaper reclaims it before any census. `Free` on every Unix: with the real fixture
kept, the paths its backstop opens are present after the helper's return, and the exact call
vector holds whichever of the helper's return and the backstop's worktree lock comes first.

## `fn the_reapers_scope_is_the_runners_identity_and_selects_its_first_probe() {`

`R7-D2`, two-process: the reaper's first listing carries exactly the probe's
`upstroke.private_root` and `upstroke.incarnation` labels as the daemon recorded them, and a
container of another incarnation under the same private root, running beside it, is neither
listed nor killed. On Linux it runs `HeldAtItsListing`: after the helper has returned, the
backstop is seen held at its listing with only that listing recorded, the real fixture's lawful
paths are seen present, and only then is the backstop let go, so the original assertions hold,
every run, over the schedule the old helper could not survive. On macOS and every other Unix it
runs `Free`, with the lawful-path assertion after the helper's return.

## `fn a_runner_whose_containers_are_unresolved_keeps_its_reaper_armed_past_its_last_handle_until_the_process_exits()`

`R6-C1`, end to end: the runtime cannot observe, stop or remove three running containers, so the
coordinator returns its error with their registrations held; while the child lives past its last
handle the relay has no call; when it exits, its reaper reclaims all three.

## `fn a_contained_run_with_its_reaper_relayed(`

An in-process width-three run over a contained runner whose reaper's program is the relay, after
the relay's self-check (`R6-D2`): the stub, run by its path, records its call where the test
reads, so a reaper's calls in this process would be seen.

The relay is written and its self-check run by isolated children of the test binary
(`FakeRuntime::install_reaper_relay`, `FakeRuntime::run_reaper_relay`), each under a deadline,
so these controls make no host launch of their own (`FUA-I4-RELAY`, PR #328's implementation
review round 4): a host launch ends in a wait for its reaper's acknowledged exit that is
unbounded by design, and made here one stopped reaper held the test, and the suite with it. The
relay's other users in this module, `a_resume_killed_inside_its_pre_flight_probe` and the
stranded-runner test, install it the same way.

## `fn a_coordinator_that_ends_disarms_its_reaper_and_kills_nothing_at_width_three() {`

`R6-D2`: a run that completes, its runner dropped: the relay holds only the self-check, and every
container was released by its own runner. Ending the reaper by end-of-file rather than `CANCEL`
(`fua-m10`) makes it list.

## `fn a_runner_whose_containers_all_ended_disarms_its_reaper_and_the_relay_is_never_called() {`

`R6-C1`'s control: the observation is lost, so the pipelines fail, but every container is
established gone; the drop disarms and the relay is never called.

## `fn every_container_an_incarnation_starts_is_covered_by_an_armed_reaper_with_its_scope() {`

The design property, in-process, through a resume's pre-flight (`resume_over` with a
`RunPreflight` over a container runner), `run_concurrently` at width three, and the width-1
`step`: every container start was covered by an armed reaper whose scope selects its labels, and
every container created was observed starting.
