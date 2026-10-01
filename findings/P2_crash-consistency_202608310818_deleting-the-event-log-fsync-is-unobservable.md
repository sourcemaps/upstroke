---
id: PR5-C-FSYNC-UNOBSERVABLE
severity: P2
disposition: deferred
category: crash-consistency
pr: 5
reviewed_sha:
location: src/events/log.rs:934
provenance: undetermined
first_bad:
guard: the change that adds a syscall-level witness of the event log's sync (a child traced for the fsync on the log's descriptor), or the project owner
---

## Failure sequence

**Deleting the `sync_all()` call in `events::log::sync_log_file` is undetectable by any test on this machine.** An fsync has no user-space observable effect: the ledger entry the suite reads would still be written, the byte length would still be the filesystem's own answer, and only a power loss could tell the difference. Every `SyncPrefix` test therefore proves that the funnel *reached* the sync and *recorded* it, not that the data reached the platter

## What the change that takes this up should do

Owner, as the ledger recorded it until PR11: PR7–PR11 implementer (the slice that owns the two-crash proof). Re-guarded below.

**Carried, not hidden.** The residual boundary is stated on the function itself (`src/events/log.rs:934`) rather than left for a reviewer to discover, and the mitigation that *is* possible is taken: the sync and its ledger entry are **one call**, because with them written as two statements a mutation that moves the `SyncPrefix` consult to *between* them puts the injection after the syscall and before the only thing that can see it — measured surviving the suite. Fused, the only place the consult can move to is after the record, where `an_injected_sync_failure_at_open_names_syncprefix_and_hands_out_no_handle` kills it. The packet names the test that would close this for real — `transaction_fault_matrix[T-PREPARED].test`'s `unsynced_merge_prepared_two_crash_barrier_before_cas_then_power_loss_keeps_log_and_ref_agreeing` — and it needs a coordinator, a CAS and a simulated power loss, none of which are PR5's

Carried in `reviews/FINDINGS.md` §2, “Open — carried deliberately, with an owner”, and confirmed still carried by the full-ledger audit of 2026-08-31 (§39). The row carried no severity label; **P2** here is this migration's judgement from the consequence described above, not the reviewer's own word.

## Re-guarded by PR11 (2026-10-01)

The guard named "PR7–PR11 implementer (the slice that owns the two-crash proof)". PR11 is the last
slice it named, and it does not close the finding, for two reasons (the PR11 record, `R-H`).
**No test closes it at the platter.** G5 established the two-crash proof for the simulated loss and
says "the physical sync is outside any test's sight"; a power loss is not something a test on this
machine can produce, so the claim the failure sequence makes — that only a power loss could tell a
deleted `sync_all()` apart — stands. **PR11 touches no byte of the Event funnel.** `src/events/log.rs`
is in PR11's frozen set and its module diff proof holds it byte-identical to the merge base
`79979d24`, so the slice cannot change what the sync does or how it is recorded.

What a test *can* observe is the request: a child traced for the `fsync` (`strace -e fsync` on Linux,
or a `ptrace`/`LD_PRELOAD` shim of the kind the build box already uses to fail syscalls) would make
deleting `sync_all()` fail a Linux test. That witness observes the call to the kernel, not the data
on the platter — narrower than this finding's own claim — so it does not close the finding by
itself, and whether it is worth building is not a scheduling decision. The guard is therefore the
change that adds such a witness, or the project owner, who decides whether the residual boundary
stays accepted.

