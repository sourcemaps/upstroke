---
id: PR330-A-DEAD-COORDINATORS-WINDOWS-REF-WRITE-CAN-LAND-AFTER-ITS-RESUME-RECLAIMED-THE-LOCK
severity: P2
disposition: deferred
category: crash-consistency
pr: 330
reviewed_sha: a0464f432f3d93134360f5d5a88e60b653cd6fb4
location: src/workspace_manager.rs:3634
provenance: pre_existing
first_bad: predates PR11; the Windows ref-lock reclaim has rested on the ambient job's kill-on-close since it landed (design/26_design_merge_queue_protocol.md:398); named R-REF in #330's design round 3 and regraded in its round 4 (reviews/2026-10-02-pr11-follow-up-c-record.md, section 4.8)
guard: before G6, a native Windows execution of a terminated update-ref against a reclaimed ref lock decides its ceiling, or the owner accepts the Windows residual as master has it; the change to the Windows ref-lock reclaim takes it up
---

## Failure sequence

Windows only. On Unix every engine `update-ref` holds the run's cleanup lease for as long as it lives
(`src/workspace_manager.rs:3553-3570`), and a resume refuses while anyone holds it. On Windows that lease holds nothing
and is never held (`src/rundir.rs:2503-2528`).

    coordinator A runs `git update-ref --no-deref <run ref> Y X` through the ref funnel
    -> Git creates `<ref>.lock`, checks that the ref still names X, writes Y into the lock, and goes to publish it
    -> A dies; its ambient job's kill-on-close terminates the update-ref child, and termination is asynchronous: "the
       terminated process cannot exit until all pending I/O has been completed or canceled"
    -> A's resume B takes the run lock, finds `<ref>.lock` naming Y, the value its own retry writes, and reclaims it
       (`reclaim_own_ref_lock`, `src/workspace_manager.rs:3634`), as `design/26` allows once "no process of the run can
       still be writing", which on Windows rests on that kill-on-close
    -> B completes X -> Y, and later Y -> Z
    -> if A's child was inside its publication when termination began, and that publication can still land after B
       deleted the lock's name, the ref goes back from Z to Y
    -> the engine's next write of that ref, or its next resume, refuses on the disagreement ("`task_merged` exists but
       the ref disagrees"); if nothing writes that ref again, the run ends with it moved back

**How Git for Windows publishes**, at v2.50.1.windows.1 (`compat/mingw.c:2845-2910`): it opens `<ref>.lock` with
`DELETE` access and `FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE`, renames it over the ref through
`SetFileInformationByHandle(FileRenameInfoEx)` with `FILE_RENAME_FLAG_REPLACE_IF_EXISTS |
FILE_RENAME_FLAG_POSIX_SEMANTICS`, and closes the handle. Once termination begins no user-mode code runs, so a
publication can land only if the thread is already inside that one call. The successor's delete of the lock's name is
not refused, because the handle shares delete.

**What is not known**, and what no documentation the record holds settles:
- whether a rename through a handle can still land after another process has deleted the file's name, under POSIX
  delete semantics;
- how long such a call can stay pending, behind a filter driver or on a network filesystem.

So the order is neither shown nor excluded. Evidence: #330's record §4.8, with its Git for Windows citations
(`~/orch-pr11/logs/pr11_fuc_design4/git-src/git-src-citations-r4.txt`) and Microsoft's `TerminateProcess`
documentation (`~/orch-pr11/logs/pr11_fuc_design2/docs/ms-terminateprocess.txt:73-75`). Design review round 3 of #330
showed that compare-and-swap does not exclude it: Git checks the expected old value under its lock and publishes later,
without checking again (`~/orch-pr11/reviews/review-330-d3-triage.md`, FUC-D3-RREF).

## Why P2, with its ceiling not established

- Reasoned, not executed. The window needs a rename in flight at the moment of termination, still pending while the
  successor starts, takes its locks, reclaims and writes twice.
- When it is reached, it fails closed at the engine's next write of that ref or at its next resume, which refuse on the
  disagreement. It stands only where nothing writes that ref again.
- If a native Windows execution shows the late rename landing after the reclaim, it is P1, and it blocks G6.
- **G6 applicability:** Q1 (the ref lock is reclaimed as residue before reuse), Q6 (one writer per ref; FIFO
  integration) and ST-18 (kills between finalization's ref deletions). Not INV-22: no row changes.
- **Pre-existing on master,** and follow-up C leaves it as master has it: neither of C's closures renames refs.

## What the change that takes this up should do

First decide the ceiling: on the Windows guest, terminate an `update-ref` child inside its publication (a filter
driver or a slow volume can hold the call), reclaim the lock as the successor does, write the ref twice, and see whether
the late rename lands. Then, if it can:
- give each engine `update-ref` child on Windows a hold the successor observes and waits out, as the Unix cleanup lease
  does, designed and executed natively;
- or reclaim a ref lock only once the dead coordinator's job is known empty, as follow-up C's quiescence option D2b′
  would report;
- or have the owner accept the Windows residual, with `design/26`'s sentence corrected to say what kill-on-close does
  not establish.

## Accepted (2026-10-11, PR #330's repair round 9): decision D5(b)

**The decision.** D5(b) is accepted: an agent decision by `pr11_decide_c` under the owner's delegation of 2026-10-11
(babysit's 0194a1e4…), in its decision report (`~/orch-pr11/logs/pr11_decide_c/DECISION-REPORT.md`, sha256 `19b080be…`,
section 3, C-3), and not the owner's own selection. Its text, the PR11 decision appendix's §7.2:

> **D5(b).** The owner accepts the Windows residual as master has it: a ref write a terminated engine `update-ref` child
> had begun publishing may land after the successor reclaimed the lock. `PR330-A-DEAD-COORDINATORS-WINDOWS-REF-WRITE-CAN-LAND-AFTER-ITS-RESUME-RECLAIMED-THE-LOCK`
> stays filed with this acceptance recorded, and `design/26` is corrected as below. [ACCEPT]

**What it changes.** This file stays filed with that acceptance recorded: it is not closed and not regraded (P2, its
ceiling not established), and its guard and failure sequence are unchanged. In the same change (#330's repair round 9)
`design/26_design_merge_queue_protocol.md:398` is corrected: the ambient kill-on-close job "does not establish that a
terminated child's pending ref rename has not landed", a window accepted as decision D5(b), under the owner's delegation
of 2026-10-11, and filed as this finding.

**The risk, preserved.**
- A ref write a terminated engine `update-ref` child had begun publishing may land after the successor reclaimed the
  lock.
- If it is reachable it is P1: on Windows, a run can end with one of its refs moved back where nothing writes that ref
  again.
- Its reachability is neither shown nor excluded. The filed P2 grades a reasoned window; it is not a ceiling.
- It applies to Q1, Q6 and ST-18.
- G6 meets this item by acceptance, not by measurement, and a later topology activation on Windows carries the window.

**What it does not do.** O7(a), the native determination, was stopped by a provider safety classifier at
2026-10-02T21:04:01Z; no result of it is claimed or implied, and nothing of it is reproduced or rerouted. (c1), a
Windows hold, is not commissioned. The acceptance becomes effective for G6 once #330's repair round 9 has passed its
review and #330 has merged: #330's record §5.5 asks for a reviewed acceptance.