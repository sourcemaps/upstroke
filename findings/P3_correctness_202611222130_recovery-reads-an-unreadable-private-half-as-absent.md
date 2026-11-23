---
id: PR402-RECOVER-PRIVATE-HALF-METADATA-ERROR-READ-AS-ABSENT
severity: P3
disposition: accepted-risk
category: correctness
pr: 402
reviewed_sha: 61c0f6e48990247f1a13a3f530ede781924a9c25
location: src/engine/topology/recover.rs:321
provenance: pre_existing
first_bad: predates #402: the check is in 5cd8747a; #402 added the commit record's digest check to the same function
guard: the sweep of src/engine/topology/recover.rs, or the next change to RecordsVerified::verify's private-half check
---

## Failure sequence

From #402 round three's review:

    a run's private half exists, but reading its metadata fails (`PermissionDenied`, `EIO`)
    -> `Path::is_dir` answers `false`, as it does for any `metadata` error
    -> `RecordsVerified::verify` refuses: the private half is at that path "and nothing is there"
    -> the operator is told the private half is missing; the I/O error is never named

The check, at `src/engine/topology/recover.rs:321`:

```rust
if !private.is_dir() {
    return Err(UpstrokeError::Refused {
        message: format!(
            "run `{run_id}` records its private half at `{}` and nothing is \
             there. A missing schema-4 private half is not recreated: the \
```

It has been there since 5cd8747a (`git blame` gives that commit for lines 321 to 325).

Why §7 reaches it. `src/engine/topology/recover.rs` is not in `standards/SWEEP.md`'s swept table, so
the activation rule (`standards/SWEEP.md:9`) governs it: §6 and §7 apply to "the whole body of any
function the change modifies". #402's a6354e4c added the `rundir::is_run_started_sha256` check to
`RecordsVerified::verify`, and the reviewer reads that as activating the whole body, this check
included. §7's bullet (`standards/07_standards_errors_and_panics.md:12`): "Inspect structured error
kinds: only an actual not-found becomes absence; permission, corruption, malformed data and
transient I/O stay errors."

Nothing has run this: no test or reproduction holds a private half whose metadata cannot be read.

## Why it is accepted

- Unwitnessed, and P3: no failing test, reproduction or mutation witness.
- §7's bullet carries no `MUST`.
- Recovery still refuses. The `Err(UpstrokeError::Refused { .. })` is returned before the owner
  record is read, so on this path nothing is deleted or recreated; the failure is a misleading
  message, not a wrong action. Inferred from the refusal path, not run.
- #402 is a P3 batch whose code runs ended at round two (ruling r42, step 7).

## What a change that takes this up should do

- Read `fs::metadata` on the private half and treat only `ErrorKind::NotFound` as absent; that case
  keeps today's refusal and message.
- Any other error refuses, naming the path and the error, not absence.
- A test with a private half that cannot be read, asserting that the refusal names the error and
  does not say "nothing is there".
