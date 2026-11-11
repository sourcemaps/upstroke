---
id: FUD-FULL-INSTRUMENTLIST
severity: P3
disposition: deferred
category: docs-contract
pr: 331
reviewed_sha: 1263f2a39dedeb12e252cfc3046149f9be9baacd
location: reviews/2026-10-02-pr11-follow-up-d-record.md:4721
provenance: introduced_by_feature
first_bad: 0ec7631b2f72355613618befe56c77bd2bc8ef78
guard: an owner-directed follow-up after #331: by the owner's instruction "we fix F1 and record the rest" (2026-10-13; babysit's direction, sha256 `df023d73…`) this stays open, neither fixed nor rejected nor accepted; a change to follow-up D's code after #331, owned by the PR11 orchestrator (`orch_pr11b`) for the owner, repairs it with a regression guard and deletes this file
---

# The record's merge-bar instrument lists omit src/runner/contract.rs

F7 of the owner's full review of #331, P3 there and here; its evidence level: reasoned as a finding; the row's necessity executed.

## Failure sequence

1. §5.17.2, §5.17.8 and §5.20.6 (`reviews/2026-10-02-pr11-follow-up-d-record.md:3430`, `:3780`, `:4449`) count
   `src/runner/contract.rs`'s payload-census row for `src/workspace.rs` (five `.env(` to seven,
   `src/runner/contract.rs:2488-2502`) as one of D's instrument rows.
2. §5.22.3(c) (`:4718-4723`) says D's change against master has "exactly D's three, `clippy.toml`,
   `effects/allowlist.toml` and `effects/wrappers.toml`", and §5.23.5 (`:4938-4939`) lists the same three.
3. The lists that classify the pull request for the merge omit an instrument path; `CLAUDE.md` calls the known
   instruments "not a checklist that clears a PR by omission".

**Executed:** setting that row back to five fails
`runner::contract::tests::every_production_command_spec_payload_is_classified`, so the row is necessary and exact; the
omission is only in the lists. It changes no outcome, since the allowlist alone already makes the merge the owner's.

## Evidence and its limits

Found by the owner-requested full review of #331 at `1263f2a3` (Fable 5.1 at `max`, read-only; CHANGES_REQUIRED, F1
P2 and F2 to F8 P3, no P0 or P1), read here only from its neutral verbatim copy
`/home/ubuntu/babysit-pr11/evidence/d-owner-full-review-1263f2a3-verbatim-20261013.md` (sha256 `137685c8…`).
That review is the evidence; this file does not re-execute it. The limits it states hold for this finding too: no
Windows, macOS or physical power-loss testing; Git 2.55.0 built by the reviewer from its tarball, the hash checked
and the signature not; four of its outputs kept through `grep` only; the PR body and CI logs not inspected; and
R-D1's proposal not staged for it.

## Category and provenance

- **Category `docs-contract`:** the record's classification text omits a path.
- **Provenance `introduced_by_feature`:** §5.22.3(c) was written in round D9's `0ec7631b`, the first bad commit,
  and §5.23.5 in round D10's `1263f2a3`.
- This pull request's body declares all four of D's instrument paths against master (`effects/allowlist.toml`,
  `effects/wrappers.toml`, `clippy.toml` and `src/runner/contract.rs`); the record's historical passages stay as
  written.

## What the change that takes this up should do

Name `src/runner/contract.rs` in the record's merge-bar instrument lists, beside the other three.

## Disposition: open, by the owner's choice

The owner's instruction "we fix F1 and record the rest" (2026-10-13), as babysit's direction carries it
(`/home/ubuntu/babysit-pr11/evidence/d-f1-repair-and-open-followups-direction-20261013.json`, sha256 `df023d73…`), repairs F1 in #331
and records F2 to F8 as open, owner-directed deferred follow-ups: neither fixed nor rejected, each keeping its P3
severity and its evidence level, and none repaired in that round. This file is that record for F7. It is not
resolved, not an accepted risk and not invalid. It stays open until a change to follow-up D's code after #331, owned
by the PR11 orchestrator (`orch_pr11b`) for the owner, repairs it with a regression guard and deletes this file
with a `fixed` ledger row.
