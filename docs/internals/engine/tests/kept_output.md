# `src/engine/tests/kept_output.rs`

Extended notes for [`src/engine/tests/kept_output.rs`](../../../../src/engine/tests/kept_output.rs).

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
