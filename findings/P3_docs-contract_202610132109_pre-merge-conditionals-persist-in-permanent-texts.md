---
id: FUD-FULL-PREMERGEWORDING
severity: P3
disposition: deferred
category: docs-contract
pr: 331
reviewed_sha: 1263f2a39dedeb12e252cfc3046149f9be9baacd
location: design/15_design_event_log_resume_run_layout.md:265
provenance: introduced_by_feature
first_bad: e845540daa35fa34146cb4a42184967582689e9f
guard: an owner-directed follow-up after #331: by the owner's instruction "we fix F1 and record the rest" (2026-10-13; babysit's direction, sha256 `df023d73…`) this stays open, neither fixed nor rejected nor accepted; a change to follow-up D's code after #331, owned by the PR11 orchestrator (`orch_pr11b`) for the owner, repairs it with a regression guard and deletes this file
---

# Conditionals true only before #331 merges stay in permanent texts

F8 of the owner's full review of #331, P3 there and here; its evidence level: reasoned.

## Failure sequence

1. `design/15_design_event_log_resume_run_layout.md:264-265` says "Nothing in this paragraph is in force until that
   pull request merges, after follow-up B's (pull request #329)", and #329 has merged; `effects/allowlist.toml:973` and
   `:1100` say "until #331 merges, the second amendment governs"; the same conditional is in
   `docs/internals/workspace.md:73-74`, `docs/internals/engine/attempt.md:15`, `coordinator.md:16` and `resume.md:11`,
   the legacy finding (`:166`) and the external-prune finding (`:400`).
2. These sentences become false the moment #331 merges, in the living design and in the instrument.
3. Neither the record (§5.23.5) nor the decision's steps 8 to 10 name who rewrites them at the merge. Most of round
   D10's edits use the timeless "in force only from #331's merge"; these do not.

## Evidence and its limits

Found by the owner-requested full review of #331 at `1263f2a3` (Fable 5.1 at `max`, read-only; CHANGES_REQUIRED, F1
P2 and F2 to F8 P3, no P0 or P1), read here only from its neutral verbatim copy
`/home/ubuntu/babysit-pr11/evidence/d-owner-full-review-1263f2a3-verbatim-20261013.md` (sha256 `137685c8…`).
That review is the evidence; this file does not re-execute it. The limits it states hold for this finding too: no
Windows, macOS or physical power-loss testing; Git 2.55.0 built by the reviewer from its tarball, the hash checked
and the signature not; four of its outputs kept through `grep` only; the PR body and CI logs not inspected; and
R-D1's proposal not staged for it.

## Category and provenance

- **Category `docs-contract`:** texts that will misstate what is in force.
- **Provenance `introduced_by_feature`:** `git log -S` finds design/15's and the allowlist's cited sentences first in
  `e845540d`, round D10's status words, the first bad commit.
- F1's repair round's own sentences carry no pre-merge conditional.

## What the change that takes this up should do

Rewrite each of these sentences timelessly (in force only from #331's merge), or name the step at the merge that
rewrites them.

## Disposition: open, by the owner's choice

The owner's instruction "we fix F1 and record the rest" (2026-10-13), as babysit's direction carries it
(`/home/ubuntu/babysit-pr11/evidence/d-f1-repair-and-open-followups-direction-20261013.json`, sha256 `df023d73…`), repairs F1 in #331
and records F2 to F8 as open, owner-directed deferred follow-ups: neither fixed nor rejected, each keeping its P3
severity and its evidence level, and none repaired in that round. This file is that record for F8. It is not
resolved, not an accepted risk and not invalid. It stays open until a change to follow-up D's code after #331, owned
by the PR11 orchestrator (`orch_pr11b`) for the owner, repairs it with a regression guard and deletes this file
with a `fixed` ledger row.
