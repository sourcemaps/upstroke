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
guard: the next change to how `upstroke decide` reads the plan file (`recorded_plan` in src/decide.rs) or to `util::read_file_bounded`
---

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

- Bound the plan read by a fixed byte count of decide's own, independent of what the file says
  about itself, and say in the prompt, as it already says of an edited plan, when the plan is
  not forwarded because it is over that bound.
- Keep the recorded-hash check whole: a plan is forwarded only when every one of its bytes was
  read and hashes to the run's `plan_hash`, so the bound can only make decide forward less.
- Make it observable with a test whose plan file is over the bound, asserting the prompt's
  not-forwarded line, and a control at the bound that is forwarded.
