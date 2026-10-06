# `src/ulid.rs`

Extended notes for [`src/ulid.rs`](../../src/ulid.rs).

The code is the authority for what it does; this file is the whole of its prose, moved out of
the source verbatim. Each section is headed by the line of code the comment sat above, spelled
as it is in the source, so the heading is the grep string that finds the code.

## Module

ULID generation (§15: `run-id = ULID`). A 48-bit millisecond timestamp and
80 bits of SHA-256 over a domain tag, time, process id and per-process nonce.
The inputs have separate fixed-width encodings, so a pid bit cannot cancel
a nonce bit before hashing. These deterministic names are not secrets or
proof of ownership. Filesystem callers must reserve new roots exclusively.

The coordinator incarnation id is the one id with a second construction
(`incarnation_ulid`, PR11 follow-up C): the same layout, its 80-bit field also
hashing 128 bits the process draws from the host, because a slot instance's
name is a function of the id that created it
(`reviews/2026-10-02-pr11-follow-up-c-record.md`, §4.2).

## `#![forbid(`

The three governed lints are `forbid` here since #318's third round: this file
stated no level for any of them and inherited none, so each took its level
from `-D warnings` alone, which an inner `allow` the placement scan does not
read -- macro-written, or spelled apart -- lowers; #318's second MAIN review
executed exactly that in `src/plan/mod.rs` and reached `std::fs::write` from
a production topology body while clippy and every governance test passed
(`GUARD-DECISION-SILENT-PRODUCTION-FILES-OUTSIDE-THE-ROLL-CALL`). A leaf with no children; its one per-site `#[expect]` is of `clippy::indexing_slicing`, not a governed lint, so the `forbid` does not reach it.
`forbid`, not `deny`, because it compiles: nothing in the file allows a governed lint, and clippy over all targets exits 0 with the fence in place. A downgrade beneath
a `forbid` is `E0453` however it is written or generated, and the enforcement
is the lint gate's -- rustc resolves no `clippy::` lint, so `cargo build` and
`cargo test` compile what clippy refuses. `effects::tests::every_unclassified_production_file_states_each_governed_lint_or_inherits_its_forbid`
names this file the day the fence is removed, and
`every_fence_of_a_governed_lint_forbids_wherever_forbid_would_compile` the
day it drops to `deny`.

## `static NONCE: AtomicU64 = AtomicU64::new(0);`

Monotonic per-process nonce: many calls can share one millisecond, so the
timestamp alone must never be the whole seed.

## `pub fn incarnation_ulid() -> String {`

The coordinator incarnation id (`RealIds::incarnation`, which the pre-lock
checks draw before any lock): `ulid`'s layout and parts, and a draw from the
host. A slot instance is named by a hash of the id that created it, and an
instance that two incarnations share is the defect PR11 follow-up C closes
(`PR329-A-RESUME-REBINDS-A-SLOT-ITS-DEAD-COORDINATORS-GIT-CHILD-STILL-WRITES`).
The clock, the pid and the per-process counter alone name neither the machine
nor the PID namespace: two processes in two namespaces, or on two machines
sharing a checkout, can draw all three equal (the record, §4.2, FUC-D3-TAGUNIQUE).
With the draw, two incarnations share an id with probability at most 2^-80, and
only within one millisecond, under SHA-256 as a random function and a host whose
draws are uniform and independent. The uniqueness is probabilistic, not absolute,
and a host that hands two processes the same randomness voids the bound; the id
then still hashes the clock, the pid and the counter, so it is never weaker than
`ulid`'s. Run ids, scratch names and staging names keep `ulid`.

## `fn now_ms() -> u64 {`

The clock reading both wrappers sample: milliseconds since the epoch, the top
of the range for a clock past it, and zero for one before the epoch.

## `fn host_draw() -> [u8; 16] {`

Sixteen bytes from the host, through the standard library's OS-seeded
`RandomState`: `BuildHasher::hash_one` of two distinct constants under one fresh
state. `RandomState::new` keys SipHash-1-3 with 128 bits each thread draws once
from the operating system — `getrandom` on Linux, `ProcessPrng` on Windows,
`CCRandomGenerateBytes` on macOS — and steps one key per new state, so it is
never a fixed key and two calls on one thread draw differently
(`library/std/src/hash/random.rs:56-88` at 1.85.0, the record's §4.2). The
standard library documents its seed as a best effort from a secure source and
warns that a seed drawn while the entropy pool is low may be weaker. No crate
is added for it: `Cargo.lock` holds no `getrandom`, `rand` or `rand_core`, and
the coordinator already depends on this source through Tokio, whose runtime
seed is `RandomState::new().hash_one(...)`.

## `fn ulid_from_parts(now_ms: u64, pid: u32, nonce: u64) -> String {`

The whole construction, over parts the caller supplies rather than the ones
`ulid` samples from the process. Splitting the sampling from the arithmetic
is what lets a test fix every input and assert an exact string, instead of
asserting a probabilistic projection of whatever the clock happened to say.

## `pub(crate) fn incarnation_from_parts(now_ms: u64, pid: u32, nonce: u64, draw: [u8; 16]) -> String {`

The incarnation's construction over parts and a draw the caller supplies,
for the reason `ulid_from_parts` exists: SHA-256 over its own domain string,
`upstroke.incarnation.v1` and a zero byte, then the big-endian timestamp, pid
and nonce, and the draw's sixteen bytes. Its own domain keeps an incarnation
id from ever equalling the run id of the same parts. Crate-visible so that
`RealIds`' test can rebuild the id the wrapper returned.

## `fn render(now_ms: u64, digest: Sha256) -> String {`

The layout both constructions share: the 48-bit millisecond prefix and the
digest's first ten bytes as the 80-bit field, in Crockford base32, twenty-six
characters.

## `fn observe_sampled_parts(_now_ms: u64, _pid: u32, _nonce: u64) {}`

Outside tests the observation seam is nothing at all: an empty call, so
`ulid` keeps the behaviour it had before the seam existed. The half that
records is `observation`, below.

## `fn observe_incarnation_parts(_now_ms: u64, _pid: u32, _nonce: u64, _draw: [u8; 16]) {}`

The same seam for `incarnation_ulid`, nothing at all outside tests.

## `mod observation {`

The recording half of the observation seam.

It is a module rather than a pair of loose items because the first
test-configured attribute in a file is where `effects::production_region`
truncates, and `effects::tests::
every_production_region_that_stops_early_stops_at_a_module` requires that
cut to land on a module. Everything above this point is the whole
construction, which is what the region is for.

## `mod observation` › `pub(super) static SAMPLED_PARTS: Cell<Option<(u64, u32, u64)>> =`

The parts of this thread's most recent `ulid` call, or `None` if it
has made none since the cell was last taken.

## `mod observation` › `pub(crate) type IncarnationParts = (u64, u32, u64, [u8; 16]);`

`(now_ms, pid, nonce, draw)`, as `incarnation_ulid` sampled them.

## `mod observation` › `pub(super) static SAMPLED_INCARNATION_PARTS: Cell<Option<IncarnationParts>> =`

The parts and the draw of this thread's most recent `incarnation_ulid` call,
or `None` if it has made none since the cell was last taken.

## `mod observation` › `pub(super) fn observe_sampled_parts(now_ms: u64, pid: u32, nonce: u64) {`

Records the three parts `ulid` has just sampled, so a test can rebuild
the id from exactly those values instead of inferring them from the id
and from `NONCE`. Inference cannot distinguish a wrapper that constructs
from what it sampled from one that constructs from something else;
capture can. The record is per-thread, so tests running in parallel
never see one another's.

## `mod observation` › `pub(super) fn observe_incarnation_parts(now_ms: u64, pid: u32, nonce: u64, draw: [u8; 16]) {`

Records the parts and the draw `incarnation_ulid` has just sampled, for the
same reason: a wrapper that zeroed its draw, or constructed from another one,
is told apart only by capture.

## `mod observation` › `pub(crate) fn take_sampled_incarnation_parts() -> Option<IncarnationParts> {`

Takes this thread's record, for this module's tests and for `RealIds`' in
`src/engine/topology/seams.rs`, which rebuilds the production incarnation from
it; re-exported at the module's top so that a sibling can name it.

## `mod tests` › `type Vector = (u64, u32, u64, &'static str);`

`(now_ms, pid, nonce, the id those parts construct)`.

## `mod tests` › `const FIRST_MS: u64 = 1_788_084_161_241;`

The millisecond of the first call in the recording below, and an
unremarkable clock reading for the boundary rows to hold fixed while
they push some other part to its edge.

## `const HASH_CONSTRUCTION_VECTORS: &[Vector] = &[`

The inputs were recorded by the old ambient-wrapper extraction. The
outputs here use the new construction, computed independently with
Python's hashlib.sha256 and big-endian integer encoding. They pin the
domain tag, field widths, digest prefix and Crockford encoding. The old
splitmix outputs remain in git history, not a compatibility promise for
newly generated ids.

## `const PARTS_AT_THEIR_BOUNDARIES: &[Vector] = &[`

The edges of each part's range, which no ambient sample reaches: a clock
at zero, at the last millisecond the field can print, and at the first
one past it; a pid at the top of its type; and high nonce bits retained
from the former construction's boundary inputs. Computed by an
implementation using hashlib.sha256 and Crockford base32 independently
of this module and validated first against every row above.

## `mod tests` › `type IncarnationVector = (u64, u32, u64, [u8; 16], &'static str);`

`(now_ms, pid, nonce, draw, the id those parts and that draw construct)`.

## `const INCARNATION_VECTORS: &[IncarnationVector] = &[`

Computed independently with Python's `hashlib.sha256` and big-endian integer
encoding (`~/orch-pr11/logs/pr11_fuc_impl/vectors/vectors.py`, which first
reproduces a row of `HASH_CONSTRUCTION_VECTORS` from `ulid`'s own
construction). They pin the incarnation's domain string, the draw's place
after the nonce, and that each of the draw's first and last bytes reaches the
id; the rows at the clock's two extremes pin the shared layout.

## `mod tests` › `fn take_sampled_parts() -> Option<(u64, u32, u64)> {`

This thread's most recent record, cleared before the call under test so
that a wrapper which stopped reaching the seam reads as absent rather
than as whatever was left behind.

## `fn ulids_do_not_collide_casually()` › `const PID: u32 = 4_242;`

One clock reading and one pid, and the single part the wrapper does
vary within a millisecond swept across two hundred consecutive values.
That is precisely the collision the nonce exists to prevent, and the
inputs now decide the outcome rather than what the clock happened to
say while the test ran.

## `fn the_public_wrapper_returns_exactly_a_parts_construction()` › `let _ = take_sampled_parts();`

The seam reports what `ulid` sampled, so all three parts arrive
independently of the id rather than being read back out of it. That
is the difference that matters: a wrapper which samples one nonce and
then constructs from another satisfies any test that infers its parts
from its own output, and fails this one.

## `fn the_public_wrapper_returns_exactly_a_parts_construction()` › `let _ = ulid();`

A second call reserves a nonce of its own, and `NONCE` only ever
increases, so this holds however many threads drew from it in between.

## `fn equal_clock_pid_and_nonce_with_different_draws_construct_different_ids() {`

T-ID1's second half (the record, §4.6): the clock, the pid and the nonce held
equal, which is the collision FUC-D3-TAGUNIQUE named, and only the draw varied.
A construction that dropped the draw makes every one of these ids equal.

## `fn the_incarnation_wrapper_returns_exactly_a_parts_and_draw_construction() {`

T-ID2: the wrapper's observation seam records the parts and the draw, and the id
is rebuilt from them. A draw of all zeros, or one that repeats on the next call,
is the mutation M-ID (a draw that is not one), and both assertions are what
turn it red; a genuine draw is all zeros with probability 2^-128.

## `fn reserving_a_nonce_yields_the_previous_value_and_wraps_at_the_top() {` › `let counter = AtomicU64::new(41);`

The reservation `ulid` makes, on a counter belonging to this test, so
the process-wide `NONCE` other tests draw from is left alone.

## `fn reserving_a_nonce_yields_the_previous_value_and_wraps_at_the_top() {` › `let at_top = AtomicU64::new(u64::MAX);`

At the top of the range it wraps rather than trapping, so a process
that draws more than `u64::MAX` ids keeps issuing them. The nonce is
one of three terms in the seed, not the whole of it.

## `fn parts_at_their_boundaries_construct_their_recorded_ids()` › `let epoch = ulid_from_parts(0, 0, 0);`

The printed field is forty-eight bits wide while the seed takes all of
`now_ms`: two clock values exactly one field apart print the same ten
leading characters, and the eighty bits under them still tell the two
milliseconds apart. This asserts that width, not the mask that spells
it out — `<< 80` into a `u128` discards bit 48 and above by itself, so
dropping the mask entirely would change no output.

## `render` › `CROCKFORD`

The mask is at most 31 and CROCKFORD has exactly 32 entries.
