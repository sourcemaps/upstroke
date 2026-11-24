# `src/events/log/premove.rs`

Extended notes for [`src/events/log/premove.rs`](../../../../src/events/log/premove.rs).

The source defines behavior; these notes hold the module's contracts and rationale.
Each code span in a section heading is an exact source fragment. Search it as a fixed string
in the linked module, using the enclosing item to distinguish repeated lines.

## Module

The pre-move `EventLog`, transcribed verbatim, as an oracle.

The obligation it serves — "byte-identical legacy behaviour … exact write/flush/sync and torn-tail
semantics" — cannot be checked by comparing the moved writer against itself, so the oracle is the
code as it stood **before** the move: `src/events.rs` at commit `ff0490a`, lines 1478-1585, with
its comments moved into this note and these mechanical changes and no others:

  * the type is `PremoveEventLog`, so both writers can be linked at once;
  * the error type is `UpstrokeError`, which `ff0490a` spelled `TactusError`;
  * `EventBody`, `Event` and `UpstrokeError` are imported rather than in scope.

To check that claim without trusting this comment:

```text
git show ff0490a:src/events.rs | sed -n '1478,1585p'
```

and compare. If a change to the funnel is *meant* to change legacy behaviour, the differential
tests in `super::tests` fail and this file is the thing that must be argued with.

## `#![allow(clippy::disallowed_methods, clippy::disallowed_types)]`

LEGACY-EFFECT: this module is in the **frozen legacy section** of `effects/allowlist.toml`, which
carries its justification and the condition under which the section shrinks.
The design packet's `decisions.effect_site_inventory.mechanism` (2), which is not in this
repository, is the source the allowlist entry's `packet` field cites for this placement; the
entry's `legacy_effect` text is the justification that applies.

## `pub struct PremoveEventLog {`

The append-only writer. One per run, held by the engine — `upstroke answer` deliberately does not
write here (it drops a file the engine ingests), so the log has exactly one writer and interleaved
lines are impossible.

## `impl PremoveEventLog` › `pub fn open(path: &Path, warnings: &mut Vec<String>) -> Result<Self, UpstrokeError> {`

Open for appending, discarding an incomplete trailing record first.

A process killed mid-write can leave a line with no newline. Appending straight after it would
splice the fragment and the next event into one unparseable line, losing both.

Terminating the fragment with a newline instead is worse than it looks: it promotes a torn *tail*,
which [`crate::events::read_all`] recovers from, into an unparseable line in the *middle*, which
[`crate::events::read_all`] must treat as a rewritten log and refuse. So the fragment is truncated
away. That is not rewriting history — those bytes are by construction an event that never
finished being written, and no reader could ever have parsed them — and it keeps "damage anywhere
but the end means corruption" a statement the reader can still trust.

## `pub fn open(path: &Path, warnings: &mut Vec<String>) -> Result<Self, UpstrokeError> {` › `match std::fs::read(path) {`

Truncate before taking the append handle, through a handle of its own. On Windows an append-only
handle is opened with FILE_APPEND_DATA and *not* FILE_WRITE_DATA, so `set_len` on it fails
outright with access denied.

## `impl PremoveEventLog` › `pub fn append(&mut self, body: EventBody) -> Result<Event, UpstrokeError> {`

Append one event and get it back **as it will be read back**.

Returning the round-tripped event rather than the one just constructed is what keeps "the log is
the source of truth" literally true. Anything the wire format cannot represent — a
sub-millisecond duration, say — must not survive in the engine's memory either, or live state
would quietly hold more than a replay could ever restore and the two would disagree in a way no
amount of care at the call sites would catch.

Flushed and synced before returning: §19 promises a crash or power loss is recoverable by
replaying this file, which is only true if the event reached the disk before the work it
describes carried on. A run emits tens of events, so the cost is noise beside a single attempt.
