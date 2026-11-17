---
id: PR369-WINDOWS-REPARSE-AS-STAGED
severity: P3
disposition: deferred
category: portability
pr: 369
reviewed_sha: 82899f00847d38d4d8802d9f6290e7beeff22b4b
location: src/rundir/ownership.rs:362
provenance: pre_existing
first_bad:
guard: the change that decides in `design/` which reparse points a regular file may carry, or any change to how the staged marker is recognised
---

# A Windows reparse point named like the staged marker reads as a regular file

## Failure sequence

Found by round five's frontier pass over pull request #369, by reading the code and Rust's
standard library. It has not been reproduced. The sequence below is the reviewer's, and no Windows
run here has made such an entry.

`unbound_shape` in `src/rundir/ownership.rs` answers a marker-less husk
`NothingBound(StagedMarkerOnly)` only when its one entry is named `.creating.tmp` and
`fs::symlink_metadata` of that name answers `FileType::is_file()` (lines 361 to 363). On Windows,
Rust 1.85's `is_file()` means "not a directory and not a symbolic link", and it counts a reparse
point as a symbolic link only when the reparse tag is a name surrogate
([implementation](https://github.com/rust-lang/rust/blob/1.85.0/library/std/src/sys/pal/windows/fs.rs#L1056-L1083),
[reparse tags](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-fscc/c8e77b37-3909-4fe6-a4ea-2b9d423b1ee4)).
So:

1. On Windows, a marker-less public husk holds one entry. It is named `.creating.tmp`, it is not a
   directory, and it is a reparse point whose tag is not a name surrogate, such as an
   `IO_REPARSE_TAG_AF_UNIX` socket.
2. The census lists the husk. The listing completes and shows that one name.
3. `fs::symlink_metadata` reports the entry without following it, and `FileType::is_file()`
   answers true.
4. `unbound_shape` answers `NothingBound(StagedMarkerOnly)`.
5. `status` reports reclaimable staged residue. The design says a `.creating.tmp` that is not a
   regular file is content and the directory is kept
   (`design/15_design_event_log_resume_run_layout.md`, "A marker-less run directory is reclaimed
   only on a complete listing"). That answer is `Retained(MarkerlessWithContent)`.

### Why it is the base's

At `71eac07c`, `unbound_shape` answered `StagedMarkerOnly` from the name alone (line 330), so an
entry of that name answered the same whatever it was, this reparse point included. This pull
request added the regular-file check (`PR369-STAGED-NAME-NOT-A-FILE`), which retains a directory or
a link of that name. It leaves this one entry kind answering as the base did.

### Inferred

- The staged marker the engine writes is a regular file (`stage_json`, `src/rundir.rs`), so only
  something outside the engine puts a reparse point at that name.
- After the reviewed SHA, the check moved into `staged_marker_is_a_regular_file` in `src/rundir.rs`.
  The proof and `remove_unbound_husk` both ask it. So when `remove_unbound_husk` looks at the name
  again before unlinking, it gets the same wrong answer. Under the plan, the census would unlink the
  entry as if it were the staged marker and then remove the directory. (At the reviewed SHA, the
  removal unlinked the name without looking.)
- Linux and macOS are not affected. There, `is_file()` is `S_ISREG`, and a socket of that name is
  already retained.
- The suite's Windows link fixtures are junctions (`mklink /J`, `src/rundir/tests.rs`). A
  junction's tag is a name surrogate, so `is_symlink()` already answers true for it, and a test
  that uses a junction cannot see this defect.
- Some ordinary file content on Windows is itself a reparse point, for example
  data-deduplication files and cloud-file placeholders. Refusing every reparse point, the way
  `is_reparse_point` does for the locator chain in the same file, could retain a real staged marker
  on such a volume. Retaining it only costs a report. Which way to go is the decision the next
  section asks for.

## What the change that takes this up should do

- Decide in `design/` which reparse points a regular file may carry when it is read as the staged
  marker: none, or a named set of tags. Every other tag is content, and the husk is retained.
- Make `staged_marker_is_a_regular_file` answer by that decision on Windows, by reading the reparse
  attribute and tag. The proof and the removal then still ask one question.
- Test it on the Windows leg with a reparse point the suite can create, one whose tag is not a name
  surrogate. Assert that the husk is retained as `MarkerlessWithContent`, and that
  `remove_unbound_husk` under `StagedMarkerOnly` leaves the entry and the directory in place.
