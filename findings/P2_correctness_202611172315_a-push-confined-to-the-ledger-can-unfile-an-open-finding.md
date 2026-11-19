---
id: PR382-ACCEPTED-P3-REPURPOSES-FINDING-FILE
severity: P2
disposition: deferred
category: correctness
pr: 382
reviewed_sha: 4a1af8fb632545bf6f6421c6f59168bf2b18ad1d
location: scripts/pr-ready-audit.sh:1388
provenance: pre_existing
first_bad: 112c4aff40c0f075348947dbb2541e79f2d3c60e
guard: the follow-up pull request `fix-P2/correctness_a-push-confined-to-the-ledger-can-unfile-an-open-finding`; until it merges, the operator's rule (ruling r20, step 6) that a ledger-only delta is enqueued only where `git diff --name-status --no-renames` shows additions alone
---

# A push confined to the ledger can unfile an open finding

## Failure sequence

The review's sequence, as the review writes it:

    an unresolved finding file exists at the reviewed head
    -> a ledger-only follow-up changes that file's `id:` to the new, unwitnessed P3 instead of
       creating a file of its own
    -> `finding_file_count` returns one for the new id
    -> the audit preserves the prior review and reports READY
    -> the old unresolved finding disappears from `findings/`

The review's witness, run in a temporary clone: the audit reports READY with `moved=ledger-only`;
the new id's file count is 1 and the old id's is 0.

Read and not run, from the ruling. Commits that touch only `findings/` keep the review
(`scripts/pr-ready-audit.sh:1192-1222` at `4a1af8fb`), and a row then needs only one file carrying
its id at the head (`:1358` for a `deferred` row, `:1388` for an accepted one), with nothing asking
what else the push did. That rule and the head-only file check are the audit's first commit's
(`112c4aff`), and `master`'s `scripts/` and `.github/scripts/` are the base's, so on `master` the
same retag under a `deferred` row reads READY, as does a ledger-only push that deletes an open
finding's file. The branch policy refuses neither: it lists added and renamed files only
(`.github/scripts/changed-in-range.sh:181`) and judges a name and a severity, never an `id:`
(`.github/scripts/validate-pr-branch.sh:3067-3125`).

**Why this merges `deferred`.** This finding carries a witness, and `MAINTAINING.md` step 5 says a
witnessed finding is fixed. It merges `deferred` in #382 on the owner's ruling r20 of 2026-11-17,
an exception for #382 only to that rule, because the defect is the base's, in lines #382 does not
change, and its repair amends step 5.

## What the change that takes this up should do

A push keeps the review only if it adds files under the ledger. One that deletes, renames or
modifies a file there reads as a repair (NEEDS-ATTEST).

The alternative: any change that keeps each file's `id:` keeps the review. Its gap is that an
unreviewed rewrite of an open finding's severity still passes.

Either amends step 5's exemption for a push confined to the ledger; the choice between them is the
owner's.

Witness: the review's retag under a `deferred` row, the same retag under an accepted row, and a
plain deletion of an open finding's file, none of them reading READY.
