# `src/engine/topology/coordinator/tests/verification_charges.rs`

Extended notes for [`src/engine/topology/coordinator/tests/verification_charges.rs`](../../../../../../src/engine/topology/coordinator/tests/verification_charges.rs).

The code is the authority for what it does; this file is the whole of its prose but for the
concurrency protocol standards §10 places at its site. Each section
here is headed by the line of code it describes, spelled as it is in the source, so the heading is
the grep string that finds the code.

## `fn the_record_kill_childs_deepest_git_dir_fits_the_windows_budget_under_the_guest_lanes_temporary_directory()`

The record-kill child's deepest `$GIT_DIR`, rebuilt from the names its run is given and judged by `add_worktree`'s own
Windows refusal, which `workspace_manager::fixture::refuse_git_dir_over_the_windows_budget` asks on any platform (the
guard and Git for Windows' rendering are compiled for tests everywhere). The child's temporary directory is named as
its parent names it, by a real `kill_dir(RECORD_KILL_TEMPORARY)`; the fixture as the child builds it, by a real
`Fixture::created(RECORD_KILL_TAG)`, which is what `Wide::durable` calls for the incarnation `inc-1`; and the worktree
as its manager places it, `slot_path` of the reviewer snapshot `k0-g0-a1-review0`, the add the guest refused. The
private root and the slot below it are read from the fixture and the manager rather than retyped. Only the temporary
directory is substituted: the guest lane's, 41 bytes as its job logs spell it. Of the slot names `naming.rs` renders,
an attempt's reviewer snapshot is the longest at equal indices, so only a reviewer snapshot with a two-digit index
would be deeper.

The add must pass under the guest lane's temporary directory and again under one `GUEST_TEMPORARY_SLACK` bytes
longer. That is the margin, 12 bytes: it holds the path at or under 208 under the guest lane's, the length it had
before #330's instance tag, and it refuses a tag of 15 bytes or more. The test reads no platform's own temporary
directory: it keeps only the names of the trees it makes, rejoins them under the fixed root, and measures the result
as Git for Windows would be handed it.

## `fn a_registry_refusal_after_two_paid_passes_is_recorded_with_both_at_width_three() {`

B's charged refusal (the O14(b) proposal's witness for B, §7): the third reviewer's snapshot meets
T2′'s torn registration, and its access refuses at its deadline as `UpstrokeError::RegistryRefused`
naming `commondir` after passes 0 and 1 were charged. The command ends with the record of both as its
last append, no terminal, and the transaction open; replay and the live totals agree, and the record
carries $0.50 of them. With the tear removed, the next process's recovery settles the sequence
interrupted after the record, its replayed spend is the live spend of the process before it, and
the resumed run completes with the passes counted once. Red at #329's merge, where the log ends at
`merge_verification_started` and the passes are charged live only.

## `fn a_registry_refusal_after_the_takeover_at_the_third_review_is_recorded_with_both_passes() {`

The refusal's third face, O9(a)'s: T15's failing filter is planted before the third reviewer's
snapshot, its checkout fails after Git took the destination over, and the access refuses at once.
The record and the resume are as at width 3. Unix only, as T15 is.

## `fn retained_attempt_finished_of_delta(body: &TopologyEventBody) -> bool {`

Delta's (task 3's) retained `attempt_finished`, where the L7 witness plants its tear.

## `fn stopped_and_recorded(`

What both `O14B2-1` witnesses assert after the stop. The record is first: replay agrees with the
live totals for the run and the verifying task, and its failure message carries the four totals and
the count of records, so a red run shows what was lost. Then `records_both_passes`. Then #329's stop,
as its own witnesses assert it: the command ends on the shutdown, whose error says that nothing was
appended but the record; the retry's wait answered the shutdown; no append failed; nothing but the
record was appended after the injection; the retried task started no attempt; no terminal, and the
transaction still open; the integration ref where the log authorizes it; the invocations balanced;
the run reopened.

## `fn sequence_endings(`

How each invocation of one sequence ended, from the recording runner.

## `fn a_shutdown_answered_inside_a_retrys_wait_records_the_passes_its_cancelled_verification_charged()`

`O14B2-1`'s L6 face, adapted from the regular lens's reproduction in the #334 delta review: its
composition, scheduling and tear. At width three a verification has charged two $0.25 passes and its
third reviewer still runs. Beta's failing gate is retried into the tear, and the shutdown is answered
inside the retry's wait. The cancelled verification's completion is discarded with both passes kept,
and `finish` records them past the stop. The third reviewer ended cancelled. After the resume, recovery
settles the sequence interrupted after the record, and the run completes with the passes counted once.
Red before the repair: the run was $1.75 live and $1.25 replayed, the verifying task $1.00 and $0.50,
with no record.

## `fn a_shutdown_answered_inside_a_retrys_wait_after_the_result_arrived_records_both() {`

`O14B2-1`'s L7 face. Four tasks: the verification's gate and both reviewers end while gamma's and
delta's gates hold snapshots, so its result arrives and is charged while the gate drains. Delta's gate
then fails, and its retry meets the tear while the gate is still draining. The shutdown answered there
ends the integration; its recording point leaves the passes to `finish`, which records them past the
stop. The rest is as the L6 witness. Red before the repair with the same totals, where the
integration's own offer was refused.
