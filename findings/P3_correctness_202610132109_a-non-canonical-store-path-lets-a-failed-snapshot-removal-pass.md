---
id: FUD-FULL-NONCANONICALSTORE
severity: P3
disposition: deferred
category: correctness
pr: 331
reviewed_sha: 1263f2a39dedeb12e252cfc3046149f9be9baacd
location: src/workspace.rs:2440
provenance: introduced_by_feature
first_bad: b9559a4e4ed63009bf93fd815c1187e7c2749dda
guard: an owner-directed follow-up after #331: by the owner's instruction "we fix F1 and record the rest" (2026-10-13; babysit's direction, sha256 `df023d73…`) this stays open, neither fixed nor rejected nor accepted; a change to follow-up D's code after #331, owned by the PR11 orchestrator (`orch_pr11b`) for the owner, repairs it with a regression guard and deletes this file
---

# A store path that is not Git's realpath lets the snapshot cleanup accept a failed removal

F3 of the owner's full review of #331, P3 there and here; its evidence level: executed.

## Failure sequence

The snapshot cleanup's attempt succeeds when `!worktree_is_registered(source_root, path, …)`
(`src/workspace.rs:2440`), which byte-compares Git's printed worktree path with the engine's (`git_path_field_matches`,
`:2571-2586`). The gate store is `$HOME/.upstroke/runs/<id>/…` with `$HOME` as given (`src/util.rs`'s
`user_upstroke_dir`, `src/rundir.rs`'s `with_private_root`), never canonicalized, and Git registers a worktree added
through a symlinked parent by its realpath. With the store behind a symlink (macOS's `/var` to `/private/var`, a
symlinked `$HOME`):

1. the removal reads another checkout's torn `commondir` and fails;
2. the list reads the registry whole and prints the realpath, which never equals the symlinked spelling;
3. the attempt returns `Ok` after one pass, and the intent is deleted, leaving the snapshot's registration of a
   deleted directory behind.

Executed with the test fixture (the script `[Torn, Whole, Whole, Torn]`): a canonical `TMPDIR` serves all four reads
and leaves no registration; a `TMPDIR` behind a symlink serves `[Torn, Whole]` and leaves the snapshot's registration
(`registration_left_behind=true`, the directory gone, no intent to reclaim it).

**A regression against master.** Master's B-W924-R1 retries the removal on its own standard error, independent of the
list: master's `a_snapshots_removal_and_its_list_are_attempted_again_past_another_registrations_empty_commondir` passes
under the symlinked `TMPDIR` (executed), and this pull request's rescripted test of that name fails
(`src/workspace.rs:5043-5048`). The kept add witness (`:5016`) fails under the symlink at master and here alike, so that
one is not this pull request's. The S witnesses are Linux-only, so no CI leg sees this.

**Impact.** A stale registration of a deleted directory for each torn removal, where the store path is not canonical;
D's (e1) and (e2) closure for the removal does not apply there. No data is lost.

## Evidence and its limits

Found by the owner-requested full review of #331 at `1263f2a3` (Fable 5.1 at `max`, read-only; CHANGES_REQUIRED, F1
P2 and F2 to F8 P3, no P0 or P1), read here only from its neutral verbatim copy
`/home/ubuntu/babysit-pr11/evidence/d-owner-full-review-1263f2a3-verbatim-20261013.md` (sha256 `137685c8…`).
That review is the evidence; this file does not re-execute it. The limits it states hold for this finding too: no
Windows, macOS or physical power-loss testing; Git 2.55.0 built by the reviewer from its tarball, the hash checked
and the signature not; four of its outputs kept through `grep` only; the PR body and CI logs not inspected; and
R-D1's proposal not staged for it.

## Category and provenance

- **Category `correctness`:** the cleanup's success test accepts a removal that failed. Its precondition, a store
  path that is not Git's realpath, is a configuration upstroke supports on every platform (a symlinked `$HOME`), not one
  platform's behaviour.
- **Provenance `introduced_by_feature`:** the byte comparison is master's (`git log -S` finds `git_path_field_matches`
  and `worktree_is_registered` only in master's history), but master's B-W924-R1 retries the removal on its own
  standard error. This pull request's option S, in its round-D7 merge `b9559a4e`, runs the removal and the list
  as one attempt of the tolerant access whose success is the list's verdict, which makes the comparison decide.

## What the change that takes this up should do

Compare canonical forms in `git_path_field_matches` or `worktree_is_registered`, or fail the attempt on the
removal's own exit status when the list cannot prove the path gone; and make the S witnesses use the fixture's canonical
path.

## Disposition: open, by the owner's choice

The owner's instruction "we fix F1 and record the rest" (2026-10-13), as babysit's direction carries it
(`/home/ubuntu/babysit-pr11/evidence/d-f1-repair-and-open-followups-direction-20261013.json`, sha256 `df023d73…`), repairs F1 in #331
and records F2 to F8 as open, owner-directed deferred follow-ups: neither fixed nor rejected, each keeping its P3
severity and its evidence level, and none repaired in that round. This file is that record for F3. It is not
resolved, not an accepted risk and not invalid. It stays open until a change to follow-up D's code after #331, owned
by the PR11 orchestrator (`orch_pr11b`) for the owner, repairs it with a regression guard and deletes this file
with a `fixed` ledger row.
