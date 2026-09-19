# `src/engine/topology/finalize.rs`

Extended notes for [`src/engine/topology/finalize.rs`](../../../../src/engine/topology/finalize.rs).
[Source on GitHub](https://github.com/sourcemaps/upstroke/blob/master/src/engine/topology/finalize.rs).
The relative link works in a checkout or on GitHub; the GitHub link also works from the published site.

The code is the authority for what it does. The explanatory prose is preserved below.
Each backticked part of a section heading is an exact source excerpt. Search for the final
excerpt within the preceding item when a heading names both an item and a line inside it.

## Module

Terminal finalization (`run_end_policy.terminal_finalization`): after a durable `run_finished`,
the report is written and the cleanup steps run in the packet's fixed order. Two callers: the
loop, once closure has appended the end (`TopologyRun::close_run`), and recovery step (b)
(`recover::finalize_if_finished`), which finalizes a Complete or Halted run found on disk and
only then refuses continuation. Both reach the same function with the barrier-proven fold, so a
finalization is idempotent across incarnations: a step that already ran finds nothing to do —
or, for a checkout's removal since PR10's round 9, only its parent directory's barrier to take
again, on the absent branch as on the present one, so a resume after a failed or unfinished
barrier converges once it holds and refuses at it with the intent standing until then
(`a_failed_checkout_barrier_is_retried_before_its_intent_is_removed`).

## `pub struct Finalize<'a> {`

What finalization needs and nothing more: the manager for the execution root and the refs, the
public run directory for the report, the run id for the ref namespace, and the proven fold with
its events for the projection.

## `pub enum CleanupStep {`

The six steps, in `ORDER`, each a row of the outcome equations: task worktrees and intents
(R9), snapshots (R24), staging (R10), the prepared and candidate-prepared pins (R12, R23), the
candidates refs (R11), and the execution root (R18) once empty. The root step first removes the
staging leftovers of `intents/` (`WorkspaceManager::remove_staging_leftovers`, through the
intent-removal funnel of each leftover's kind): reclaim reports and never removes them, since no
filename proves who wrote a file, but the finalizer holds the run lock and the cleanup lease, so
no writer of this root is alive and the ownership proof is in hand — a leftover the emptied
root would otherwise keep is what blocked the pruning in the round-2 crash lens's P2-3.

## `impl CleanupStep` › `pub const fn applies_to(self, outcome: &RunOutcome) -> bool {`

Per outcome, from `resource_accounting.rows[*].at_run_end`. Snapshots, staging, pins and the
root are pruned at every outcome. Task worktrees are pruned here only at Complete and Halted:
at Parked and BudgetExceeded every generation closure closed has already been scrubbed with its
close (record §3 R10), and nothing open is left. The candidates refs are Complete's alone —
"Parked: resumably_open (never pruned)", "Halted: persistent_output (forensic; listed in
report.json)", "BudgetExceeded: resumably_open (never pruned)".

## `pub struct Finalized {`

What finalization reports back: the outcome it finalized, whether the report was written or
found current, what each step removed, whether the root went, how many candidates refs the
report lists as retained, the linked-worktree registrations the removals passed over, and the
staging-shaped entries under the run directory that no record of this run's names, which the
report write passed over and left as found (since PR10's round 10; both are named in step (b)'s
refusal) —
entries of the repository's store that name no checkout (`WorkspaceManager::WriterProof`),
sorted, without duplicates, left as found.

## `pub fn finalize(`

Refuses without a durable `run_finished` — nothing is written and nothing deleted for a run
that has not ended. Otherwise the report is derived from the fold and written when the file on
disk is missing or stale — a file whose stored digest is not the digest of its own bytes, or
whose outcome or runner is not the derived report's (`TopologyReport::is_fresh_against`) — then
the steps run. The write is `rundir::write_report`'s staged, synced, renamed publication — the
report staged inside a directory the write makes for itself under a name it first records
durably in the run's private half, which is why finalization carries the private directory too
(`Finalize::private`, from the locked root and the run's paths) — so a
report present under its name holds durable bytes and the refs the steps prune go only after it
(DESIGN.md §26); a report found current is not written again, and its directory's barrier is
taken again (`rundir::sync_report_dir`, through the report's two sites exactly as the write is —
`Report.Write` inside `RunDir.WriteReport` around the barrier — so the typed inventory holds the
branch's one external effect and each of its four coordinates is selectable on a restart: the
ST-18 matrix's cells fault the first finalization and its restarts observe the fresh branch's
coordinates unarmed, and `fresh_report_hook_errors_stop_cleanup_and_retry` arms each of the four
on a restart, for both outcomes, and holds the injected error to stop cleanup — until PR10's
round 5 the branch reached no site, the round-5 contract lens's F1, and until round 6 only
`Report.Write`'s before phase was ever faulted there, the round-6 crash lens) before the first
pruning effect, at Complete and at Halted alike: the rename that made it current was made after its bytes were synced, so the
bytes are durable, but the *name* is durable only once the directory's barrier completed, and a
first finalization may have died or failed between the rename and that barrier — until PR10's
round 4 the fresh branch pruned behind a name proven visible, not durable (the round-4 crash lens,
P2; the two barrier tests below drive both outcomes since round 5, the round-5 crash lens's P1;
`the_report_is_durable_before_any_ref_is_pruned_and_a_current_report_is_not_rewritten`,
`a_report_directory_barrier_that_fails_refuses_pruning_on_every_resume_until_it_holds`,
`a_report_rename_without_directory_sync_is_proven_before_pruning`).
A fault at any site ends the command with the steps before it done and the steps after it not
started; the next resume's step (b) runs the whole order again and converges (ST-18,
`kill_after_report_before_each_cleanup_step`, which at every cell of the matrix — both phases of
every effect's site, the run lock's release included — asserts what has and has not been
removed at the fault, and
`a_kill_inside_finalization_after_the_execution_root_is_removed_converges_on_the_next_resume`,
which kills a real child inside finalization).

## `pub fn refuse_continuation(run_id: &str, finalized: &Finalized) -> UpstrokeError {`

Step (b)'s refusal, worded with what the finalization it follows did: the report regenerated or
already current, each step's count, and — when there were any — the registrations passed over,
by path, so an operator learns what Git cannot list, prune or repair and this run did not
delete.

## `fn scrub_slots(`

One namespace of the execution root at a time, by slot kind, through the Worktree and Snapshot
funnels' remove-then-remove-intent pairs. The removal runs under
`WriterProof::NoWriterAlive` — the finalizer holds the run lock and the run's cleanup lease, so
no writer of the root is alive — and what it passes over is collected for `Finalized`. Since
PR10's round 8 the removal makes the checkout's deletion durable inside its own site, before the
funnel returns: `WorkspaceManager::remove_worktree_proving` syncs the checkout's parent directory
(the slot kind's directory under the root, recorded as the ledger's `SyncedDirectory`) after the
tree is gone, so the intent that names the checkout — removed next, and synced in its own
directory — can never outlive a deletion the disk rolled back; a resume enumerates the intents,
and a checkout with no intent would have kept the root non-empty on every later resume (the
round-8 crash lens, P2). Since round 9 the barrier is taken on the absent branch as well — a
checkout already gone is not proof that the attempt which removed it reached its barrier, or that
the barrier held, and a retry that read absence as proof removed the intent behind an unproven
deletion (the round-9 crash lens, P2; syncing a directory whose entry is already gone is the proof
the first attempt did not give — and where the slot kind's directory is itself absent, a root an
older engine or a fixture made without the scaffolding `create_execution_root` lays down, the
execution root above it is synced instead, so the directory's own absence and the checkout's with
it is what is made durable, `sync_slot_directory_absent`) — its error is the funnel's and never absorbed (the round-9 crash
lens, P1: with `.unwrap_or(())` on the barrier both round-8 guards still passed, since neither
armed the checkout's parent), and the barrier's own record carries the checkout observed absent
in the statement before it (`util::EntryObserved`; the round-9 fix-check lens, P1: with the sync
moved ahead of the removal the round-8 guard still passed). The five guards in the recover tests:
`a_checkouts_deletion_is_made_durable_before_its_intent_is_removed` (the parent synced between the
removal's two phases and before the intent site's first, and the barrier's record showing the
checkout absent at the instant of the sync),
`a_failed_checkout_barrier_is_retried_before_its_intent_is_removed` (the parent's barrier armed to
fail across two resumes at both outcomes, each ending at the barrier's diagnostic with the intent
still present, the third converging) and `an_absent_checkout_retries_its_parent_barrier` (the
funnel called twice under the fault: the retry meets an absent checkout and refuses again), and
`an_absent_slot_directory_is_made_durably_absent_in_the_root_before_the_intent_is_removed` (the
checkout removed by a first resume whose barrier was refused, the emptied slot kind's directory
then removed by hand: the next resume syncs the execution root with that directory observed
absent, removes the intent and finalizes) and, since round 10,
`a_failed_execution_root_barrier_is_retried_before_the_intent_is_removed` (the same state, the
execution root's barrier then armed to fail across two resumes at both outcomes, each ending at
the barrier's diagnostic with the intent still present, the third converging — rule 2 for the
barrier round 9 added, whose guard dropped its fault before the root barrier ran and read only the
record, which the helper writes whether or not the barrier held, so the swallow
`outcome.unwrap_or(()); Ok(())` at the helper's end passed it; the round-10 lenses, P1). Guards
rather than cells in the matrix, since the barrier is inside the removal's site and the matrix
already faults both its phases; the round-8 lens's alternative — a recovery that rediscovers an
intent-less checkout — was not taken, because it would add a second reader of the root's contents
beside the intents where one barrier in the funnel every caller already uses (the live loop's
scrubs and reclaims included) removes the shape.

A torn registration no longer wedges the scrub (`PR5-RD-002-ENGINE-RECLAIM-LOOPS`). A `git worktree
add` killed while it writes `commondir` leaves that file zero-length, and Git's enumeration, which
each `remove_intent` runs before it acts, dies on it. The scrub goes one slot and one kind at a
time, so a torn registration of a slot it had not reached, or of a kind a later scrub owns, refused
it at the first intent removal on every attempt. `WorkspaceManager::remove_intent` now runs, when
that enumeration refuses, the forced removal of every other slot an intent names whose registration
is torn — its checkout and registration go, its intent stays for the scrub that owns it — and asks
Git again. Nothing here changed: the witnesses are in this module's `mod tests`, which drive this
function directly over the same-kind and the cross-kind fixture, stop it at each of its phases, and
pin the refusal of a torn registration no intent names.

## `fn delete_refs_under(`

Every ref under a prefix of the run's namespace, deleted expected-old at the value just read,
through the Ref funnel's site for that pin or ref kind.

## `mod tests`

Finalization's `scrub_slots`, driven directly over a real repository, a real execution root and the
workspace manager's own fixture, for `PR5-RD-002-ENGINE-RECLAIM-LOOPS`: a `git worktree add`
killed while it writes `commondir` leaves that file zero-length, Git's enumeration then dies before
it emits any record, and every `remove_intent` revalidates through that enumeration. The scrub
removes one slot's worktree and then its intent, one slot kind at a time, so before the intent
removal learnt to repair a torn registration another intent names, a torn registration of a slot
the scrub had not reached, or of a kind a later step scrubs, refused the scrub at the first intent
removal on every attempt.

`scrub_slots` is private, so it is tested here, in its own module's test module, rather than made
visible for a test. It is inline, as `identity`'s and `seams`' are; a whole-file one would also have
to be entered in `WHOLE_FILE_TEST_MODULES`, the census list under `src/effects/`. The suite builds
nothing of the run around it: the scrub takes a manager, the topology hooks and a slot-kind filter,
and `NoTopologyHooks` is the smallest `TopologyHooks` there is. Every byte the suite puts on disk
goes through the workspace manager's fixture — `tear_registration` writes the torn registration,
`git_os` adds the checkout no intent names — because `src/engine/topology/**` is a topology module
and `std::fs`'s writing half is on the clippy denylist in tests too. The reads (`tree_bytes`, the
enumeration's refusal) are `std::fs`'s reading half and Git's own answer.

## `mod tests` › `fn scrub(`

`scrub_slots` with a fresh list for the registrations it passes over, and the assertion that it
passed over none: every registration these fixtures build names a checkout.

## `mod tests` › `fn add_snapshot(fixture: &Fixture, sequence: u64) -> (Slot, PathBuf) {`

A snapshot slot through the snapshot store's own funnel, at the fixture's head, and where its
checkout is.

## `mod tests` › `fn tree_bytes(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {`

Every entry under `root`, directories as empty entries and files with their bytes, so that
"untouched" is asserted byte for byte.

## `mod tests` › `fn assert_enumeration_dies_on(fixture: &Fixture, admin: &Path) {`

Git's enumeration dies on the registration at `admin`, read by its signature — the message names
the administrative directory and `commondir` — and never by the word the platform prints for errno
0 (`Success` on glibc, `Undefined error: 0` on macOS).

## `mod tests` › `struct CrossKind {`

One task slot, healthy, and one snapshot slot whose registration is torn: the torn slot is of a
kind the task scrub does not reclaim, so at `dfab458b` the task scrub refused at the task intent's
removal whatever the order, and the snapshot scrub that would have repaired it came after.

## `mod tests` › `fn scrub_slots_converges_past_a_torn_registration_of_the_kind_it_reclaims() {`

Fixture V8 of `PR5-RD-002` under the task scrub: `alpha` and `bravo`, `bravo`'s registration torn
and sorting second. One scrub converges: both intents, both checkouts, `bravo`'s registration, and
Git enumerates again. At `dfab458b` the scrub refused at `alpha`'s intent removal with Git's
enumeration failure on `bravo`'s `commondir`.

## `mod tests` › `fn scrub_slots_repairs_a_torn_registration_of_a_kind_a_later_step_reclaims() {`

`CrossKind` under the two scrubs finalization runs, task then snapshot. The task scrub converges:
`alpha`'s intent removal runs the snapshot slot's forced removal first, so the snapshot's checkout
and registration go and its intent stays. The snapshot scrub, the step that owns the slot, then
finds nothing to remove and converges — the state the repair leaves, taken up by its owner.

## `mod tests` › `fn scrub_slots_converges_when_git_has_pruned_the_emptied_registration_store() {`

Why the repair takes the checkout with the registration. Two task slots and a torn snapshot: the
task scrub repairs the snapshot at `alpha`'s intent removal, and `beta`'s forced removal then runs
`git worktree prune`, which deletes `<common git dir>/worktrees` once it is empty. A repair that
removed only the registration would leave the snapshot's checkout behind with no registration
directory at all, and the snapshot scrub's forced removal refuses that shape (`Io` on
`.git/worktrees`, `NotFound`) on every attempt — measured with a registration-only repair while
this test was written. With the forced removal as the repair, the checkout is already gone and the
snapshot scrub converges with no store at all.

## `mod tests` › `struct StopEffects {`

Stops the scrub at the `stop`-th hook phase it consults, by an error return, and logs every phase
it saw. `Before` is consulted before a primitive runs and `After` once it has returned, and nothing
between the stop and the scrub's return writes, so the stop leaves on disk what a kill at that
phase leaves; it does not model a power loss.

## `mod tests` › `struct StopHooks {`

`StopEffects` as the effect hooks, and `NoTopologyHooks` for the other four families, which a
scrub does not consult.

## `mod tests` › `fn a_cross_kind_scrub_stopped_at_any_phase_converges_on_the_next() {`

`CrossKind` under both scrubs, stopped at each phase in turn. At every stop no checkout or
registration is left without its intent, and the next pair of scrubs converges. The phase log of
the run that completes pins the order: the task's removal, then the snapshot's removal under its
own site as the task intent's repair, then the task intent's removal, then the snapshot scrub's
removal, which finds nothing, and its intent's.

## `mod tests` › `fn scrub_slots_still_refuses_a_torn_registration_no_intent_names() {`

The fail-closed half at the engine level: a torn registration of a checkout in the root's `tasks/`
namespace that no intent names is not the scrub's to remove, so the scrub refuses on Git's
enumeration of it, the registration and its checkout are left byte for byte, and the scrubbed
slot's intent outlives the refusal. It holds at `dfab458b` too. The checkout's path reaches Git as
the bytes it is (`git_os`).
