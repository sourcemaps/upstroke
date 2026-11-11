---
id: FUD-FULL-OWNERWORDING
severity: P3
disposition: deferred
category: docs-contract
pr: 331
reviewed_sha: 1263f2a39dedeb12e252cfc3046149f9be9baacd
location: effects/allowlist.toml:858
provenance: introduced_by_feature
first_bad: e845540daa35fa34146cb4a42184967582689e9f
guard: an owner-directed follow-up after #331: by the owner's instruction "we fix F1 and record the rest" (2026-10-13; babysit's direction, sha256 `df023d73…`) this stays open, neither fixed nor rejected nor accepted; a change to follow-up D's code after #331, owned by the PR11 orchestrator (`orch_pr11b`) for the owner, repairs it with a regression guard and deletes this file
---

# The adopted allowlist texts still attribute the choice to the owner

F5 of the owner's full review of #331, P3 there and here; its evidence level: reasoned.

## Failure sequence

1. Five adopted `[[legacy]]` texts say "AMENDED ONCE, on the owner's decision to …", and the composed
   `src/workspace.rs` text "on the owner's decision to unfreeze the module for this one change"
   (`effects/allowlist.toml:858`, `:898`, `:937`, `:1021-1022`, `:1335`).
2. The comment above each entry, `design/15`, the notes and the findings say the adoption was an agent decision by
   `pr11_decide_d` under the owner's delegation, and the decision report forbids reading it as the owner's own selection
   (its §4 step 4(a)).
3. The instrument's text, which is what stays on master, contradicts its own marker.

The decision adopted the texts "exactly as given", so this is a tension for the orchestrator to resolve (edit those
five phrases under the same authority that sanctioned edit A3, or state the carve-out), not a slip of round D10.

## Evidence and its limits

Found by the owner-requested full review of #331 at `1263f2a3` (Fable 5.1 at `max`, read-only; CHANGES_REQUIRED, F1
P2 and F2 to F8 P3, no P0 or P1), read here only from its neutral verbatim copy
`/home/ubuntu/babysit-pr11/evidence/d-owner-full-review-1263f2a3-verbatim-20261013.md` (sha256 `137685c8…`).
That review is the evidence; this file does not re-execute it. The limits it states hold for this finding too: no
Windows, macOS or physical power-loss testing; Git 2.55.0 built by the reviewer from its tarball, the hash checked
and the signature not; four of its outputs kept through `grep` only; the PR body and CI logs not inspected; and
R-D1's proposal not staged for it.

## Category and provenance

- **Category `docs-contract`:** the defect is a text that misstates who decided.
- **Provenance `introduced_by_feature`:** the phrases are this pull request's texts, written while O8 was proposed as the
  owner's decision (`git log -S` finds the resume entry's in `45b1a7b1`). They became contradictory in `e845540d`
  (round D10), which put the agent-decision markers beside them; that commit is the first bad one.
- F1's repair round added one sentence and one comment line to the resume entry and left these five phrases as they
  are.

## What the change that takes this up should do

Make the five phrases state the adoption as the markers do, under the authority that adopted the texts, or state
the carve-out beside them.

## Disposition: open, by the owner's choice

The owner's instruction "we fix F1 and record the rest" (2026-10-13), as babysit's direction carries it
(`/home/ubuntu/babysit-pr11/evidence/d-f1-repair-and-open-followups-direction-20261013.json`, sha256 `df023d73…`), repairs F1 in #331
and records F2 to F8 as open, owner-directed deferred follow-ups: neither fixed nor rejected, each keeping its P3
severity and its evidence level, and none repaired in that round. This file is that record for F5. It is not
resolved, not an accepted risk and not invalid. It stays open until a change to follow-up D's code after #331, owned
by the PR11 orchestrator (`orch_pr11b`) for the owner, repairs it with a regression guard and deletes this file
with a `fixed` ledger row.
