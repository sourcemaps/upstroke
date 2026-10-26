---
id: FUD-FULL-STATUSTABLE
severity: P3
disposition: deferred
category: docs-contract
pr: 331
reviewed_sha: 1263f2a39dedeb12e252cfc3046149f9be9baacd
location: reviews/2026-10-02-pr11-follow-up-d-record.md:64
provenance: introduced_by_feature
first_bad: 0ec7631b2f72355613618befe56c77bd2bc8ef78
guard: an owner-directed follow-up after #331: by the owner's instruction "we fix F1 and record the rest" (2026-10-13; babysit's direction, sha256 `df023d73…`) this stays open, neither fixed nor rejected nor accepted; a change to follow-up D's code after #331, owned by the PR11 orchestrator (`orch_pr11b`) for the owner, repairs it with a regression guard and deletes this file
---

# The follow-up D record's status table stops at round D8

F6 of the owner's full review of #331, P3 there and here; its evidence level: reasoned.

## Failure sequence

1. The record's §0 status table (`reviews/2026-10-02-pr11-follow-up-d-record.md:52-64`) ends at D8 (`:64`), and
   its last rows say "PROPOSED, conditional on O8 and on B-W924-R1-S, not adopted".
2. §5.23.2 (`:4881-4883`) says "From 2026-10-11 the status is this section's", but §0 does not say so.
3. A successor reads §0 first (`:5`) and takes the superseded status as the current one.

## Evidence and its limits

Found by the owner-requested full review of #331 at `1263f2a3` (Fable 5.1 at `max`, read-only; CHANGES_REQUIRED, F1
P2 and F2 to F8 P3, no P0 or P1), read here only from its neutral verbatim copy
`/home/ubuntu/babysit-pr11/evidence/d-owner-full-review-1263f2a3-verbatim-20261013.md` (sha256 `137685c8…`).
That review is the evidence; this file does not re-execute it. The limits it states hold for this finding too: no
Windows, macOS or physical power-loss testing; Git 2.55.0 built by the reviewer from its tarball, the hash checked
and the signature not; four of its outputs kept through `grep` only; the PR body and CI logs not inspected; and
R-D1's proposal not staged for it.

## Category and provenance

- **Category `docs-contract`:** the record's status statement is stale.
- **Provenance `introduced_by_feature`:** the record is this pull request's. `git log -S` puts the D8 row in
  `5b69e2ef`; the first round section the table has no row for is round D9's §5.22, `0ec7631b`, the first
  bad commit.
- F1's repair round adds no record section and leaves §0 as it is.

## What the change that takes this up should do

Add D9 and D10 rows to §0, or a one-line pointer there to the section that holds the status.

## Disposition: open, by the owner's choice

The owner's instruction "we fix F1 and record the rest" (2026-10-13), as babysit's direction carries it
(`/home/ubuntu/babysit-pr11/evidence/d-f1-repair-and-open-followups-direction-20261013.json`, sha256 `df023d73…`), repairs F1 in #331
and records F2 to F8 as open, owner-directed deferred follow-ups: neither fixed nor rejected, each keeping its P3
severity and its evidence level, and none repaired in that round. This file is that record for F6. It is not
resolved, not an accepted risk and not invalid. It stays open until a change to follow-up D's code after #331, owned
by the PR11 orchestrator (`orch_pr11b`) for the owner, repairs it with a regression guard and deletes this file
with a `fixed` ledger row.
