---
id: PR435-PLAN-READ-BOUND
severity: P2
disposition: deferred
category: performance
pr: 435
reviewed_sha: 5ca9531f56e6eb0483bb43f6fb01e3672a0dab44
location: src/decide.rs:613
provenance: introduced_by_feature
first_bad: introduced with `upstroke decide` on this branch (PR #435); the read is the same at 5ca9531f and after the round-two repair, where it is `recorded_plan` (src/decide.rs:800)
guard: the change that closes this finding: since 2026-11-27 `upstroke decide` reads no plan file (`recorded_plan` is gone from src/decide.rs), so nothing is left to bound
---

## At this branch's head: the read is gone

The narrowing of `upstroke decide` under the owner's decision of 2026-11-27 on item 52 removed
the read this finding bounds, rather than bounding it. `recorded_plan` and its
`util::read_file_bounded` call are gone from `src/decide.rs`: the prompt is built from the run's
log and from the frozen plan `engine::topology::observe` reads and checks against the run's
`normalized_plan_digest`, and no plan file of the checkout is opened. A plan file that differs
from the recorded plan is withheld and the reviewer is still asked
(`decide::tests::plan_with_an_added_paragraph_break_is_withheld`). So the sequence below no
longer happens at the head; it is kept as it was found, and this file waits only to be closed.

## Failure sequence

From PR #435's round-two review, filed under ruling R79 at P2. The review's text is not in this
repository; the sequence below is read from the code at the reviewed head and at the repair.

    upstroke decide <run-id> asks about a question
    -> the plan file is read at <repo>/<RunStarted4.plan_path> by util::read_file_bounded
    -> read_file_bounded's bound is the length the file's own metadata reports when it is opened
    -> so the whole file is read into memory, however large it is
    -> only after that does the prompt cut it to INPUT_BOUND (64 KiB) for the reviewer
    -> a plan path holding a file of gigabytes is read whole before any call is started

At 5ca9531f, `src/decide.rs:612`:

```rust
fn read_bounded(path: &Path) -> String {
    match crate::util::read_file_bounded(path) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
```

After the round-two repair, `src/decide.rs:799`, the same read, now followed by a hash of every
byte read against the run's recorded `plan_hash` (`PR435-UNVERIFIED-PLAN-INPUT`):

```rust
fn recorded_plan(path: &Path, plan_hash: &str) -> String {
    match crate::util::read_file_bounded(path) {
        Ok(bytes) if crate::ir::content_hash(&bytes) == plan_hash => {
```

The bound, `src/util.rs:286`:

```rust
pub fn read_file_bounded(path: &Path) -> std::io::Result<Vec<u8>> {
    use std::io::Read as _;
    let mut file = std::fs::File::open(path)?;
    let bound = file.metadata()?.len();
    let mut bytes = Vec::new();
    file.by_ref().take(bound).read_to_end(&mut bytes)?;
    Ok(bytes)
}
```

After the repair the read comes after admission and the reviewer's pre-flight and before the run
is claimed (`src/decide.rs:357`), so what it costs is memory and time before the first call, with
nothing written.

Not run here: no test reads an oversized plan, and no measurement was made. A plan path that
names something other than a regular file (a FIFO opens and waits for a writer) was not tried
either.

## What the change that takes this up should do

- Close this finding: with no plan file read, there is no read to bound, and
  `plan_with_an_added_paragraph_break_is_withheld` holds that no byte of the checkout's plan file
  reaches the reviewer.
- Should a later change read a plan file again, bound that read by a fixed byte count of
  decide's own, independent of what the file says about itself; forward a plan only when every
  one of its bytes was read and is the plan the run recorded, so the bound can only make decide
  forward less; and make it observable with a plan file over the bound beside a control at it.
