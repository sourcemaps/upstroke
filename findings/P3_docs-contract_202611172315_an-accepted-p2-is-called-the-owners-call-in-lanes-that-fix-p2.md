---
id: PR382-POLICY-DOC-MANUAL-CLAIM-OVERBROAD
severity: P3
disposition: deferred
category: docs-contract
pr: 382
reviewed_sha: 4a1af8fb632545bf6f6421c6f59168bf2b18ad1d
location: MAINTAINING.md:40
provenance: introduced_by_feature
first_bad: 5a6548744fbed24a203d6e06cc2a9d81962629e1
guard: the follow-up pull request `fix-P2/correctness_a-push-confined-to-the-ledger-can-unfile-an-open-finding`; until it merges, the operator's rule (ruling r20, step 6) that a ledger-only delta is enqueued only where `git diff --name-status --no-renames` shows additions alone
---

# An accepted P2 is called the owner's call in lanes that fix P2

## Failure sequence

The review's sequence, unwitnessed:

    a standards-lane review reports an unwitnessed P2
    -> the author records it `accepted-risk`, expecting the owner's call the documents describe
    -> the audit checks the lane's fix set before it reads the row
    -> it emits `open-P2` and NOT-READY
    -> the documented disposition path is unavailable

The ruling's reading: the sentence "A P2 or higher recorded as `accepted-risk` is still the owner's
call" is right about who decides, and loose about the lanes whose fix set includes P2.

## What the change that takes this up should do

Make the sentence at `MAINTAINING.md:40`, `findings/README.md:73` and `findings/PROCESS.md:572`
except a severity the lane must fix: a P2 or higher recorded as `accepted-risk` is the owner's call
only where the lane does not require it fixed.
