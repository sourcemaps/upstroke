---
id: AUDIT-MOVE-INTO-LEDGER-KEEPS-REVIEW
severity: P2
disposition: deferred
category: security-trust
pr: 433
reviewed_sha: 6513c22d24f8404b6877a6da578deaff19aa7224
location: scripts/pr-ready-audit.sh:1192
provenance: pre_existing
first_bad: 112c4aff40c0f075348947dbb2541e79f2d3c60e
guard: a pull request of its own that makes the audit's movement read list both ends of a rename; it edits scripts/, so its merge is the owner's; until it merges, the operator's rule (rulings r20 step 6 and r104) that a ledger-only delta is enqueued only where git diff --name-status --no-renames shows additions alone, all under findings/
---

# A push that moves a file into the finding ledger keeps the review

## Failure sequence

1. After a review, a push moves a file from outside the ledger into the `findings` directory:
   a rename.
2. `scripts/pr-ready-audit.sh:1192` lists the paths the push touched with
   `git log --no-merges --name-only`, and with Git's default rename detection that lists only the
   move's destination.
3. `:1218` counts the destination, under the ledger, as ledger, and `:1222` sets
   `moved=ledger-only`.
4. With no other blocker, `audit_state` (`:530-538`) prints READY. The review is kept, though the
   push took a file outside the ledger out of the tree.

**How each step is known.**

- **Run:** the #433 design note's experiment E1, in a throw-away repository on Git 2.43.0: after
  `git mv src/lib.rs findings/lib.md` and a commit, the read of line 1192 lists `findings/lib.md`
  alone, while `git diff --name-status --no-renames` lists `A findings/lib.md` and `D src/lib.rs`
  (`~/selfhost-design-20261127/audit-movement-read-experiments/run.log:4-8`, SHA-256
  `dbb0d9b6cc47b3f5746a1bbae0d082d98f1679da67fc14b6cf08b40a1dff4309`; the script, `run.sh:26-32`
  beside it, SHA-256 `210700c5ac724d7db6efedb9f53de618c8707c0242f6ddf806504a2d326b8ac4`).
- **Reported:** #433's round-four frontier pass ran master's audit, as at `3b04c1d7`, on the same
  kind of move, an unchanged `backlog.md` moved into the ledger, with an unrelated unwitnessed
  `fixed` row: it printed NOT-READY with `fixed-but-head-unmoved:DOC-ISSUE`, reading the move as
  ledger-only (`~/selfhost-auditgate-20261125/evidence/review-frontier-9740f68.md:53`, SHA-256
  `4cfc8014fce084b82512e6dc1a4b0f6e3c83b9554e6da106370ff12d584b6101`).
- **Not run:** READY with both real contexts, on a pull request.

**Meanwhile.** The operator's check of ruling r20 step 6 lists the push with
`git diff --name-status --no-renames`, which shows the deletion beside the addition (E1's listing
above), and enqueues a ledger-only delta only where it shows additions alone; ruling r104 adds that
every path it lists must be under the ledger.

## Why P2 security-trust

Graded by ruling r104:

- The same rename reading was `PR269-R1-001`, P1 `security-trust` in #269's ledger: a pull request
  that moves a source file into the ledger left the tree under a low review, because the
  changed-path listing printed only the destination. `08c80be6` fixed that listing with
  `--no-renames` (`.github/scripts/changed-in-range.sh:91-103`). The audit's own read, there since
  `112c4aff` (line 156 of the audit at `112c4aff`), was left as it was.
- P2, not P1: the policy gate refuses a file a pull request adds or renames under the ledger unless
  its name starts `P0_` to `P3_` and its frontmatter severity is one of P0 to P3
  (`.github/scripts/changed-in-range.sh:181`, `.github/scripts/validate-pr-branch.sh:3098-3116`),
  and the audit reads READY only with both required contexts green
  (`scripts/pr-ready-audit.sh:1030-1032`). So the push must be shaped on purpose, and CI must stay
  green without the moved file. `master` has no top-level tests directory.
- `PR207-AUDIT-WRITE-WINDOWS-OPEN`
  (`findings/P2_security-trust_202609061500_audit-label-and-enqueue-windows-cannot-close.md`), a
  false readiness in the same script, is filed P2 `security-trust`.

## Related, not the same

- `PR382-ACCEPTED-P3-REPURPOSES-FINDING-FILE`
  (`findings/P2_correctness_202611172315_a-push-confined-to-the-ledger-can-unfile-an-open-finding.md`):
  a ledger-only push keeps the review and can unfile an open finding.
- `PR433-ARCHIVED-LEDGER-KEEPS-REVIEW`
  (`findings/P2_correctness_202611280711_the-archived-ledgers-name-keeps-a-review-on-any-branch.md`):
  an added file at the archived ledger's name keeps the review.
- Unlike those two, this one takes a file outside the ledger out of the tree.

## What the change that takes this up should do

- List both ends of a rename, so that such a move reads `repairs`.
- Whether the move then satisfies a `fixed` row is the owner's to decide first (#433's round-four
  finding `PR433-CROSS-PREFIX-RENAME-GAINS-FIXED-CREDIT`, on its candidate).
- Its witness is the move itself, read as `repairs`; it need not reach READY.
