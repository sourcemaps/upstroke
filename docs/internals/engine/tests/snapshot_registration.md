# `src/engine/tests/snapshot_registration.rs`

Extended notes for [`src/engine/tests/snapshot_registration.rs`](../../../../src/engine/tests/snapshot_registration.rs).

## `fn the_v1_conductor_runs_and_resumes_on_the_graph_its_own_workspace_wrote()` › `run_replacement_witness_child`

The helper runs in a child because what it measures reads the environment
this process was started in: `HostEnvironment::from_process()` is
production's own read of it, and the v0.1 workspace's Git children inherit
it. There is no seam to filter either at, so the environment is prepared
instead —
[`without_ambient_replacement_controls`](../../../../src/workspace_manager/fixture.rs)
states what it takes away and why, and
`assert_replacement_controls_pinned` refuses in the child if any of it
survived.

The spawn itself was this module's own function until round 4. It is
`fixture::run_replacement_witness_child` now, shared with
`src/workspace_manager/tests.rs` and `src/gates.rs`, because three copies of
a door is three places a later repair can reach two of.

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

## `fn the_gate_role_probe_stand_in_writer_leaves_no_writer_in_another_threads_fork() {`

Linux only. The deterministic witness of pull request #368's repair of
`CI-FLAKE-GATE-PROBE-TEXT-FILE-BUSY`, described under the gate role's test
below. It runs `gate_probe_writer_helper` in an isolated child through
`agent::proc::test_support::run_test_isolated`, with no added environment
variables and a 180-second deadline, and asserts that the child succeeded and that its output
reports "1 passed", as `runner::host::tests`'
`the_reaper_relay_writer_leaves_no_writer_in_another_threads_fork` does for the
container relay writer. The child keeps the oracle's held fork away from the
other tests of this process. The gate role's section below names this test as
the deterministic witness of the repair's writer half. With `write_the_gate_probe_stand_in`
writing through `fs::write(git, script)` instead of
`runner::container::write_program_in_its_own_process`, it fails, and the child's
output carries the oracle's "the fork inherited a shim writer" with a
`WouldBlock` read; with the helper put back, it passes.

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

The cases and their assertions run in `the_gate_role_probe_cases_child`, an
isolated child that this test starts through
`agent::proc::test_support::run_test_isolated` under a 300-second deadline,
naming in `GATE_PROBE_TREE` (`UPSTROKE_W924_GATE_PROBE_TREE`) the scratch
directory the child works in, and the test asserts that the child passed and
recorded that it reached the end of its cases. The child writes the stand-in
through `write_the_gate_probe_stand_in`, never through a descriptor of its own
process: `runner::container::write_program_in_its_own_process` writes it from a
process of its own, because a fork another thread made while this process held
it open for writing kept that writer until it execed, and the gate role's exec
of the stand-in failed with "Text file busy"
(`CI-FLAKE-GATE-PROBE-TEXT-FILE-BUSY`), and `fs::set_permissions` then gives it
mode 0700, as before that fix (the helper makes it 0755). The helper's end
waits for its reaper without a bound. This test's call to it is bounded, by the
isolated child it runs in and that child's deadline (`FUA-I4-RELAY`). Before #368
the test wrote the stand-in with `fs::write`; only #368's first review round
called the helper from the test's own process. Other callers of the helper are
not all bounded: ordinary tests call it with no outer bound at nine sites,
through `git_shim`, `a_fake_cmd` and `delivery_program` and two Docker-stub
tests in `src/engine/tests.rs`, through `capability_control` (two sites) and one
Docker-stub test in `src/runner/container/tests.rs`, and one Docker-stub test in
`src/runner/container/exec/tests.rs`, recorded as finding
`PR368-WRITER-HELPER-UNBOUNDED-CALLERS`.

The two CI failures this repair answers each recorded "Text file busy". CI run
41984755598, job 127102847397, `test (ubuntu-latest)`, 2026-11-05: the test
panicked at `src/engine/tests.rs:11125:9` in the case `own-registration`, where
the probe's record held
`gate worktree-true FAIL Err("Text file busy (os error 26)")`
and the case expected
`FAIL Err("fatal: failed to read /r/.git/worktrees/upstroke-gates-1/commondir: Success")`;
3676 passed, 1 failed. CI run 42976030457, job 130375210467, `test (ubuntu-latest)`,
2026-11-11: the test panicked at `src/engine/tests.rs:11155:9` in the case
`signal`, with three probes failing `Err("Text file busy (os error 26)")`; 3736
passed, 1 failed. The race those runs met needs a fork made by another thread
of the same test process while the stand-in is open for writing. This repair
has two halves, and either one closes that window: the isolated child, which
`run_test_isolated` runs with `--test-threads=1` (`src/agent/proc.rs`), so that
no sibling test thread exists in it to fork while the write is open; and the
writer helper, which never opens the stand-in in the test's process. Putting
`fs::write` back while the cases stay in the isolated child would not bring that
race back; putting the cases back into the shared test process with `fs::write`
would. The second half has a cost: the helper runs through the process funnel
once for each stand-in the child writes, and each such run ends by waiting for
its reaper; if one stalls, the child is ended at its 300-second deadline and the
test fails. A plain `fs::write` inside the isolated child has no such path. The
helper is kept as a defence that does not depend on the test staying isolated.
When `write_the_gate_probe_stand_in` writes the stand-in from the test's own
process, `the_gate_role_probe_stand_in_writer_leaves_no_writer_in_another_threads_fork`
(above) fails deterministically: it runs that writer under the inherited-writer
oracle of `runner::host::tests`. That oracle needs a fork taken while the
writer's descriptor is open. `fs::write` opens, writes and closes a regular file
in one call. A write to a regular file can block when the device has to act, and
a file on a filesystem that can hold a write could be used to place a fork
inside it, but this tree has no practical, portable way to hold the stand-in's
write open while another thread forks, so the witness points the same writer at
a FIFO instead of the stand-in. The host oracle makes the window deterministic by
blocking the writer on a FIFO. A FIFO cannot be executed (`execve` refuses it
with `EACCES`), so that oracle cannot end in `ETXTBSY`. The host suite's
`ETXTBSY` control opens a writer of its own, so it passes whichever writer made
the file.

## `fn the_v1_include_names_the_managed_repository_however_its_path_is_spelled() {`

One run per shape, each over a repository with the blob, tree and commit
replacements: a path with `[x]*?` (`[x]` on Windows), spaces and a quote in it; a
separate Git directory; a linked worktree as the entry; on Unix, a symbolic link
as the entry and a `.git` that is a symbolic link to the Git directory; on
Windows and macOS, the entry spelled in upper case. Every role of every run read
the recorded graph, and afterwards the operator's Git still reads the
replacements.

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
