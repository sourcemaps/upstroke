---
id: PR345-T-C2-STEP-25-LOCK-UNOBSERVED-BEFORE-TIMEOUT
severity: P2
disposition: deferred
category: correctness
pr: 345
reviewed_sha: 713dc01f6b8f8249593ed217bb3e76b3e61ff912
location: src/runner/host/tests.rs:8415
provenance: introduced_by_feature
first_bad: a377f2dcf2e40231cf91e9da98470beaaa47d668
guard: a follow-up change to T-C2 (slot_roles_under_an_inherited_context_address_only_their_own_work_tree) that observes the gate's lock before requesting cancellation, under a separate bounded watchdog
---

## Failure sequence

PR #345's review 1 (the regular lens, gpt-6-astra at max) derived this from the source at `713dc01f`. No build, test or
reproduction ran.

1. **The gate role runs with a fixed bound.** T-C2's step 25 (`src/runner/host/tests.rs:8411-8418`) runs
   `git --version; sleep 60 | git update-index --add --stdin` with a five-second timeout. It asserts only that the role
   timed out.
2. **The start-up is slow.** Before `git update-index` starts, the start-up, `git --version` included, takes longer than
   five seconds.
3. **The kill is correct, and no lock exists.** The runner times out and kills the command, and `killed.timed_out`
   passes. No process took the gate's `index.lock`.
4. **The parent fails the test.** It requires `managed/.git/worktrees/gate/index.lock` to exist
   (`src/runner/host/tests.rs:8150-8158`). So correct environment isolation fails the test: a false failure, not a false
   pass.

**Why:**
- Nothing observes the lock being taken before the kill.
- So the fixed bound stands in for the readiness signal that standards §12 asks for.
- The timeout alone does not show that an index operation was interrupted.
- This finding establishes no cause of any historical CI failure.

## What the change that takes this up should do

- **Observe the gate's `index.lock`,** or another published readiness signal, before cancelling the command.
- **Do it under a separate bounded watchdog,** which fails with a diagnostic if the lock never appears.
- **Keep the assertion** that the killed gate's lock is in its own snapshot's administrative directory.
