---
id: FUD-FULL-STAGEDGITLINK
severity: P3
disposition: deferred
category: correctness
pr: 331
reviewed_sha: 1263f2a39dedeb12e252cfc3046149f9be9baacd
location: src/engine/resume.rs:634
provenance: introduced_by_feature
first_bad: 45b1a7b1aa91b2a9393fa001f5e361db81a99da7
guard: an owner-directed follow-up after #331: by the owner's instruction "we fix F1 and record the rest" (2026-10-13; babysit's direction, sha256 `df023d73…`) this stays open, neither fixed nor rejected nor accepted; a change to follow-up D's code after #331, owned by the PR11 orchestrator (`orch_pr11b`) for the owner, repairs it with a regression guard and deletes this file
---

# A staged path the guarded discard leaves in place is reported as discarded

F2 of the owner's full review of #331, P3 there and here; its evidence level: executed, on Git 2.43.0 and 2.55.0.

## Failure sequence

1. A legacy worker leaves a Latin-1 text file, `agent-output.txt` and a nested repository with a commit.
2. The coordinator's capture (`git add -A`) stages all three, so the real index holds a gitlink; the review diff fails
   because the text is not UTF-8, and N1 pins.
3. On resume, `discarded` is taken from `git status --porcelain` before G4 (`src/engine/resume.rs:564`):
   `A  agent-output.txt`, `A  latin1.txt`, `A  nested`.
4. G4 reverts the two files, leaves the gitlink by design and resets the real index, so
   `KeptDiscard::left_in_place`, the status after the revert (`src/workspace.rs:1335`), holds `?? nested/`: a
   different string.
5. The string comparison at `src/engine/resume.rs:634-639` filters nothing out. The warning says "discarded 3
   uncommitted path(s) … A  nested", followed by "1 uncommitted path(s) remain in this checkout … `?? nested/`"; the
   log's `run_resumed.discarded` is `["A  agent-output.txt","A  latin1.txt","A  nested"]`; `upstroke status` renders
   "3 uncommitted path(s) discarded" (`src/status/render.rs:74-79`); and the checkout afterwards is `?? nested/`.

**Why it is untrue.** `docs/internals/engine/resume.md:262` says the discard warning and `RunResumed.discarded` "list
the paths found less those G4 left in place", and R-D5's finding
(`findings/P3_correctness_202610031548_a-resume-that-stops-before-its-report-names-no-kept-pin.md:62`) says the same.
T-X10 (`an_unreadable_leftover_and_a_nested_repository_are_left_in_place_and_named`, `src/engine/tests.rs:14540`)
asserts it, but with a shape that died before its capture, whose index is unstaged, so both strings match.

## Evidence and its limits

Found by the owner-requested full review of #331 at `1263f2a3` (Fable 5.1 at `max`, read-only; CHANGES_REQUIRED, F1
P2 and F2 to F8 P3, no P0 or P1), read here only from its neutral verbatim copy
`/home/ubuntu/babysit-pr11/evidence/d-owner-full-review-1263f2a3-verbatim-20261013.md` (sha256 `137685c8…`).
That review is the evidence; this file does not re-execute it. The limits it states hold for this finding too: no
Windows, macOS or physical power-loss testing; Git 2.55.0 built by the reviewer from its tarball, the hash checked
and the signature not; four of its outputs kept through `grep` only; the PR body and CI logs not inspected; and
R-D1's proposal not staged for it.

## Category and provenance

- **Category `correctness`:** a durable record, `run_resumed.discarded` in the run's event log, and the operator's
  warning and `upstroke status` name a path as discarded that the resume left in place. Nothing is lost.
- **Provenance `introduced_by_feature`:** the `left_in_place` filter is this pull request's G4. `git log -S'left_in_place'`
  over `741aa3bd..1263f2a3` finds it first in `45b1a7b1` (implementation round 3), and master has no such field.

## What the change that takes this up should do

Compare by path (`git status --porcelain=v1 -z` paths) or derive both lists from one state, and add a test whose
leftover is a staged gitlink.

## Disposition: open, by the owner's choice

The owner's instruction "we fix F1 and record the rest" (2026-10-13), as babysit's direction carries it
(`/home/ubuntu/babysit-pr11/evidence/d-f1-repair-and-open-followups-direction-20261013.json`, sha256 `df023d73…`), repairs F1 in #331
and records F2 to F8 as open, owner-directed deferred follow-ups: neither fixed nor rejected, each keeping its P3
severity and its evidence level, and none repaired in that round. This file is that record for F2. It is not
resolved, not an accepted risk and not invalid. It stays open until a change to follow-up D's code after #331, owned
by the PR11 orchestrator (`orch_pr11b`) for the owner, repairs it with a regression guard and deletes this file
with a `fixed` ledger row.
