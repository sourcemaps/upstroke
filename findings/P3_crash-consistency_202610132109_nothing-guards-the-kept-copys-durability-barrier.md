---
id: FUD-FULL-COPYBARRIER
severity: P3
disposition: deferred
category: crash-consistency
pr: 331
reviewed_sha: 1263f2a39dedeb12e252cfc3046149f9be9baacd
location: src/workspace.rs:1664
provenance: introduced_by_feature
first_bad: 45b1a7b1aa91b2a9393fa001f5e361db81a99da7
guard: an owner-directed follow-up after #331: by the owner's instruction "we fix F1 and record the rest" (2026-10-13; babysit's direction, sha256 `df023d73…`) this stays open, neither fixed nor rejected nor accepted; a change to follow-up D's code after #331, owned by the PR11 orchestrator (`orch_pr11b`) for the owner, repairs it with a regression guard and deletes this file
---

# Nothing in the repository guards the kept copy's durability barrier

F4 of the owner's full review of #331, P3 there and here; its evidence level: executed (a mutation; the order traced); not a demonstrated power-loss event.

## Failure sequence

`write_copy` makes a resume's copy of the kept pin durable before the revert: `sync_all` of the `.partial`
(`src/workspace.rs:1664`), `rename` (`:1668`), then `sync_parent` (`:1672`).

1. A scratch copy with `sync_all` removed and `sync_parent` made a no-op keeps all 321 engine and workspace tests
   green; the record says the same (`no-file-sync` and `no-dir-sync` turn no test red, §5.17.4).
2. So a refactor that drops the barrier, or moves it after the revert, ships unseen.
3. A crash after the revert could then leave the only copy of what the discard removed not yet durable, which is
   R-D1-3's core guarantee.

The order is right today: an `strace` of `a_resume_writes_and_syncs_its_own_copy_before_the_revert` shows
`openat(.partial)`, `fsync = 0`, `rename`, `openat(dir)`, `fsync = 0`, and only then the first revert-phase Git child.
On Windows `sync_parent` is a no-op (`:2331-2333`), so the directory entry's durability there is reasoned, as the notes
say.

## Evidence and its limits

Found by the owner-requested full review of #331 at `1263f2a3` (Fable 5.1 at `max`, read-only; CHANGES_REQUIRED, F1
P2 and F2 to F8 P3, no P0 or P1), read here only from its neutral verbatim copy
`/home/ubuntu/babysit-pr11/evidence/d-owner-full-review-1263f2a3-verbatim-20261013.md` (sha256 `137685c8…`).
That review is the evidence; this file does not re-execute it. The limits it states hold for this finding too: no
Windows, macOS or physical power-loss testing; Git 2.55.0 built by the reviewer from its tarball, the hash checked
and the signature not; four of its outputs kept through `grep` only; the PR body and CI logs not inspected; and
R-D1's proposal not staged for it.

## Category and provenance

- **Category `crash-consistency`:** the property at risk is the copy's durability before the revert, against a crash;
  the finding is its missing guard, not a present loss.
- **Provenance `introduced_by_feature`:** `write_copy` and its barrier are this pull request's (`git log -S'fn
  write_copy'` finds them first in `45b1a7b1`, implementation round 3); `sync_parent` itself is master's.

## What the change that takes this up should do

A source-order pin for `write_copy`, in the style of the notes pin the module already has, or a test-only seam that
records the calls.

## Disposition: open, by the owner's choice

The owner's instruction "we fix F1 and record the rest" (2026-10-13), as babysit's direction carries it
(`/home/ubuntu/babysit-pr11/evidence/d-f1-repair-and-open-followups-direction-20261013.json`, sha256 `df023d73…`), repairs F1 in #331
and records F2 to F8 as open, owner-directed deferred follow-ups: neither fixed nor rejected, each keeping its P3
severity and its evidence level, and none repaired in that round. This file is that record for F4. It is not
resolved, not an accepted risk and not invalid. It stays open until a change to follow-up D's code after #331, owned
by the PR11 orchestrator (`orch_pr11b`) for the owner, repairs it with a regression guard and deletes this file
with a `fixed` ledger row.
