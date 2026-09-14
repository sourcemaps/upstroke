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
finalization is idempotent across incarnations: a step that already ran finds nothing to do.

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
report lists as retained, and the linked-worktree registrations the removals passed over —
entries of the repository's store that name no checkout (`WorkspaceManager::WriterProof`),
sorted, without duplicates, left as found.

## `pub fn finalize(`

Refuses without a durable `run_finished` — nothing is written and nothing deleted for a run
that has not ended. Otherwise the report is derived from the fold and written when the file on
disk is missing or stale — a file whose stored digest is not the digest of its own bytes, or
whose outcome or runner is not the derived report's (`TopologyReport::is_fresh_against`) — then
the steps run. The write is `rundir::write_report`'s staged, synced, renamed publication, so a
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
moved ahead of the removal the round-8 guard still passed). The three guards in the recover tests:
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
absent, removes the intent and finalizes). Guards
rather than cells in the matrix, since the barrier is inside the removal's site and the matrix
already faults both its phases; the round-8 lens's alternative — a recovery that rediscovers an
intent-less checkout — was not taken, because it would add a second reader of the root's contents
beside the intents where one barrier in the funnel every caller already uses (the live loop's
scrubs and reclaims included) removes the shape.

## `fn delete_refs_under(`

Every ref under a prefix of the run's namespace, deleted expected-old at the value just read,
through the Ref funnel's site for that pin or ref kind.
