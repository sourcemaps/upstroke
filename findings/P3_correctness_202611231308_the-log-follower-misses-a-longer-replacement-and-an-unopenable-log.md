---
id: PR407-LOG-FOLLOWER-MISSES-A-LONGER-REPLACEMENT
severity: P3
disposition: deferred
category: correctness
pr: 407
reviewed_sha: f9685666d13a931115e9e96f42b6f6b5c7e97c60
location: src/events/log.rs:1068
provenance: pre_existing
first_bad: (predates this pull request; not traced)
guard: the next change to LogTail::poll, or to upstroke status --follow
---

# The log follower misses a longer replacement and treats an unopenable log as empty

Line numbers are in `src/events/log.rs` at `f9685666`.

## What `LogTail::poll` does

`LogTail::poll` (`:1068-1096`) follows a log from a byte offset it keeps between calls:

- if `File::open` fails, it returns no events and no error (`:1073-1075`);
- if the log's length is below the offset, it resets the offset to 0 (`:1077-1080`), and it detects a
  changed log by that length test alone;
- otherwise it seeks to the offset (`:1085`) and reads the rest of the file with `read_to_end` (`:1086-1087`).

## Failure sequence

Round one's frontier pass on #407 (comment 7102049179) reported this; it is reasoned, not run.

1. `poll` reads a log up to offset N.
2. The log is atomically replaced by a longer valid log whose first record ends at N.
3. The next `poll` sees a length above N, so it does not reset the offset (`:1077`); it seeks to N (`:1085`) and returns
   only the suffix that follows it.
4. `status --follow` never shows the replacement's first events.

## Two related facts

- A log that cannot be opened (permission, a removed directory) is reported as no new events, not as an error
  (`:1073-1075`).
- `read_to_end` (`:1087`) is not bounded, unlike the module's whole-log reads through `read_file_bounded` (`:237`,
  `:287`, `:886`, `:978`).

## Severity

P3: `status --follow` is a display. The log on disk is unchanged, and a whole-log reader, `read_all` (`:970`), sees
every event.

## What the change that takes this up should do

- Detect a replacement by identity (inode or a first-line digest) as well as by length.
- Report an open failure other than `NotFound`.
- Read through `read_file_bounded` or a bounded read.
