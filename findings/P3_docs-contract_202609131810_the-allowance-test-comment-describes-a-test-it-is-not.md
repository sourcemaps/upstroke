---
id: PR282-ALLOWANCE-TEST-COMMENT-DESCRIBES-A-TEST-IT-IS-NOT
severity: P3
disposition: deferred
category: docs-contract
pr: 282
reviewed_sha: 5ca7abcd3104f92923ca76389e96f54131fa04e3
location: src/rundir/classify.rs:896
provenance: introduced
first_bad: 5ca7abcd3104f92923ca76389e96f54131fa04e3
guard: "deferred: either the comment is corrected to say the expectation is derived from INTERRUPTED_ALLOWANCE, or the assertion is changed to the literal the comment claims; the second is what the comment argues for, because as written the test passes with the constant set to 0"
---

## Failure sequence

`src/rundir/classify.rs:896-897`, above `the_interruption_allowance_is_finite_and_does_not_refill`:

> The count is asserted against a literal rather than against `INTERRUPTED_ALLOWANCE`, because a
> test that derives its expectation from the constant it is checking grows with any mutation of it
> and catches none. The constant is named only in the floor.

`:913` is:

```rust
assert_eq!(spent, u64::from(INTERRUPTED_ALLOWANCE));
```

**The expectation is derived from the constant, and the constant is named in the assertion itself.**
The comment states the exact hazard the code then exhibits, and its final clause — *"named only in
the floor"* — is false at the line below it.

**Consequence, by reading:** with `INTERRUPTED_ALLOWANCE` set to `0`, `Allowance::new()` is
`Self(0)`, the `while allowance.spend()` loop never runs, `spent` stays `0`, and
`assert_eq!(0, 0)` passes, as does `assert!(!allowance.spend())`. This test does not catch that
mutation. It is caught only by `a_burst_of_interruptions_inside_the_allowance_is_absorbed`, which
lives in a **different file** and is not named here — so the comment's claim that this test bounds
the constant is the load-bearing part, and it is the false part.

The `1_000_000` literal in the loop is a runaway ceiling, not the count under test.

Found by the `tencent/hy4-preview` fix-check lens and independently by its regression half; **not**
raised by `moonshotai/kimi-k3`, which read the same file and called this test sound.

## Why this is filed rather than fixed

Editing a tracked source file moves the head and discards a three-lens review of record.
`ORCH-P1.md` step 4: P3s are fine; no blocking P1s and no goal-blocking P2s ends the loop, **no
clean-up round**. Step 5 files residue under `findings/`, which keeps the review.

**This one is worth fixing first of the three**, because unlike the other two it names a real
weakness in a guard rather than only a miscount.
