---
id: PR433-ARCHIVED-LEDGER-KEEPS-REVIEW
severity: P2
disposition: deferred
category: correctness
pr: 433
reviewed_sha: 6513c22d24f8404b6877a6da578deaff19aa7224
location: scripts/pr-ready-audit.sh:1218
provenance: pre_existing
first_bad: 6174843b7abcaa2435d8dba3426c257ece319a41
guard: the change that carries out the owner's choice for the archive names, retiring the exemption or keeping it only where the reviewed tree holds the path; it edits scripts/ and MAINTAINING.md step 5, so its merge is the owner's; until it merges, the operator's rule (rulings r20 step 6 and r104) that a ledger-only delta is enqueued only where git diff --name-status --no-renames shows additions alone, all under findings/
---

# The archived ledger's name keeps a review on a branch cut after it left master

## Failure sequence

As #433's round-one frontier pass gives it, run on #433's candidate `c0a81fd3`:

1. A branch cut after the closed ledger left `master` (2026-11-24) receives a passing review.
2. A follow-up push adds `reviews/FINDINGS.md`.
3. The audit's walk exempts that path by name, without asking when the branch was cut
   (`scripts/pr-ready-audit.sh:1218` on `master`), so the push reads `moved=ledger-only`.
4. The audit reports READY, and makes its merge call under the stale review.

Its witness, in the pass's words: "Ran the exact-head audit with local Git history based on 50c11ded
and the existing offline GitHub stubs. The post-review addition produced moved=ledger-only, READY,
and one merge call to the stub." It ran on `c0a81fd3`, at that commit's line 1243
(`~/selfhost-auditgate-20261125/evidence/review-frontier-c0a81fd.md:12-20`, SHA-256
`b583448d9efc7ea5a039e33a0b214c208b69f602afd717b4917d89fe3ea53e83`). On `master` the exemption at
line 1218 is read, not run.

## Severity

Raised P1 by #433's round-one frontier pass; filed P2 by ruling r104. What lands unread is one
Markdown file at a retired path, where the audit counts no finding (line 460 of the audit lists
finding files only under the two ledger directories), and no open pull request carries a reviews
directory. `MAINTAINING.md:316-317` has the owner reclassify a P1 whose failure needs speculative
preconditions; rulings r37 and r85 did so.

## Where the exemption comes from

- `6174843b` (#417, the review records leaving `master`) narrowed step 5's exemption to the ledger
  and wrote that the template and the audit "still name the closed ledger's old path in this
  exemption too, harmlessly: only a branch cut before it left master can touch that path." The
  witness above disproves that, so this finding's first bad is `6174843b`. Step 5 now reads
  "`findings/`, or `reviews/FINDINGS.md` on a branch cut before that ledger left master"
  (`MAINTAINING.md:163-165`).
- The pull request template's exempt-only line names `reviews/FINDINGS.md` without that condition
  too (`.github/pull_request_template.md:41`).
- The operator's check of ruling r20 step 6 alone passes this push, since an added
  `reviews/FINDINGS.md` is an addition alone; ruling r104 adds that every path it lists must be
  under the ledger.

## What the change that takes this up should do

The owner chooses between two: retire the exemption for the archive names, so that a change under
the reviews directory loses the review; or keep it only where the reviewed tree holds the path.
Either edits the audit's line 1218 and `MAINTAINING.md` step 5 together, and its merge is the
owner's.
