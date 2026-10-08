---
id: APX-R14SPEND
severity: P1
disposition: deferred
category: correctness
pr: 329
reviewed_sha: 5c222ff2162da28aa471433773bf4966a883a428
location: src/engine/topology/run.rs:289
provenance: pre_existing   # at master; B adds the `RegistryRefused` routes (its refusals at the deadline and at once, and R14's after the takeover)
first_bad:
guard: the O14(b) change (RULINGS Q-0 to Q-3, owner-adopted 2026-10-08), before F's final merge, G6, PR12 and any topology activation; carried for B alone by the owner's O14-BF ruling, not a blocking B-merge finding during the interim; filing is not a waiver or a fix; resolved only through `findings/PROCESS.md` on the implemented and validated remedy, never automatically when O14(b) merges
---

## Failure sequence

Reasoned from the code at master `5c222ff2`, where the decision appendix found it (`DECISION-APPENDIX.md`
`0576723d`, §7.1, the O14 entry, :1021-1056: APX-R14SPEND, P1, round 2), with the routes the O14(b) proposal traces
(`O14B-MECHANISM-PROPOSAL.md` `ebeefaf4`, §2.2, :321-352: L3 and L5). The line numbers below are master's.

Width 1 (the proposal's L3):

    a verification's first review pass returns and is charged: `SpendAccount::charge` adds its cost to the run's and
      the task's in-memory `Spend` and its record to `charged` (`src/engine/topology/run.rs:440-446`;
      `Judge::judge` charges at `src/engine/topology/attempt.rs:1101`)
    -> a later pass fails after that charge, with an error that is not `UpstrokeError::Git`: B's
       `UpstrokeError::RegistryRefused` from that pass's snapshot add (`attempt.rs:1034-1036`) — a fault of the
       registry or of the whole repository that outlasted the access's deadline, the same refused at once when the
       add's destination cannot be proved untouched, or, under O9(a), an add that failed after Git may have taken its
       destination over; or, already at master, any other error `verified` passes on (below)
    -> `run::verified` keeps `charged` only inside `Verified::Unavailable`; its last arm passes the error on without
       them (`src/engine/topology/run.rs:289`)
    -> the frozen `src/engine/topology/integrate.rs` passes it on by `?` (`:696-703`), and the step ends the command
       with nothing appended
    -> the next resume's recovery appends `merge_verification_interrupted` (`src/engine/topology/recover.rs:1129-1141`),
       whose payload is `sequence` and `detail` only (`src/topology/events.rs:1000-1005`), and `Spend::replay` charges
       nothing for it (`src/engine/topology/select.rs:58-96`)
    -> the resume verifies again under a new sequence and pays for its passes again; each such refusal and resume
       lets the passes it had charged escape the run's and the task's ceilings, and the event log records none of them

Width above 1 (L5): the coordinator charges an accepted completion (`src/engine/topology/coordinator.rs:1470`), then
`verified` answers `Err`, `self.fail(error)` takes it (`:1476-1479`) and `finish` returns it appending nothing
(`:1583-1592`); the resume continues as above.

**The other errors that take L3 and L5 after a charge** (the proposal, :339-349): F's closure-1 refusals at its
placements 1 and 2; and at master, a later pass whose agent no adapter answers (`attempt.rs:1037-1046`), a pass whose
process count disagrees, refused after its own charge (`:1104-1114`), a later pass's snapshot add failing with anything
but `UpstrokeError::Git` (an injected fault is `UpstrokeError::Refused`), a reviewer's prompt that cannot be
materialized (`src/review.rs:467`), and a later reviewer ended unresolved at width 1 (`review.rs:549-551`). Not after a
charge: closure 1's `candidate_diff` and C1-4 run before the judge, and a gate's refusal precedes every review pass.

**The contract it breaks** (`design/26_design_merge_queue_protocol.md` at master): `:602-608`, under which the
unavailable terminal carries what its verification charged, because "a restart forgets what the parked and deferred
verifications of the incarnation it replaces cost, so every incarnation admits integration a ceiling had already
refused and the overspend compounds once per restart"; and `:617-622`, under which a later pass's failure "settles the
sequence unavailable rather than ending the command".

**What is executed and what is reasoned.** The sequence above is reasoned from the code, by the appendix and the
proposal, and is not executed for this file. Its class was executed once on the ledger route:
`PR272-R2-DESIGN-26-OVERPROMISES-UNAVAILABLE` (P3, at `abde1396`, the docs-contract face of this defect, kept as
filed) let an integration review return costing $2.50 and then failed registration, and recorded
`unavailable_terminals=0, transaction_open=true, live=3.7, replay=1.2`.

**At B's head** (`492325c4`) the same arms stand: `verified`'s last arm is `src/engine/topology/run.rs:779` and its Git
arm `:769`; `attempt.rs` (`:1035`, `:1101`) and `recover.rs` (`:1132-1139`) are master's, byte for byte. B adds the
`RegistryRefused` routes above; it changes nothing else of the loss.

## What the change that takes this up should do

Implement O14(b) as the owner adopted it on 2026-10-08: the proposal `ebeefaf4` with its K1 correction, RULINGS Q-0 to
Q-3 and their texts and errata, so that the passes a verification charged reach the log before its command ends on
such an error, and the resume's settlement charges them (the proposal's §3). Carry B's charged-refusal witness on its
final integrated range, keep F's associated witness obligation visible, and replace the interim O14-BF texts
`design/26` carries (B, 2026-10-08) when it lands. Resolve this file through `findings/PROCESS.md` on that implemented
and validated remedy; it is not resolved by O14(b) merging alone.

**Outside it:** attempts, which charge nothing per pass (`ATTEMPT-PAID-REVIEW-CHARGED-NOWHERE-WHEN-THE-JUDGEMENT-FAILS`,
P2); the unknown spend of a process that dies holding a verification (design/26's crash table); and the docs-contract
face `PR272-R2-DESIGN-26-OVERPROMISES-UNAVAILABLE`, which stays as filed.

## The interim, its grade and its record

- **The grade** is the appendix's: P1. Disposition `deferred`.
- **The owner's B-only carry** (the B-first decision `b-first-owner-decision-ready-20261007.md` `5dd8875c`, item 3,
  selected by the owner's answer of 2026-10-08): "For B alone, authorize carrying this exact known obligation to O14(b)
  before G6; it is not a blocking B-merge finding during this interim. This is the owner's specific temporary
  classification, not the proposition that filing alone satisfies the merge bar, and not a general P1 waiver."
- **Filing is not a waiver or a fix.** No other finding or review bar is relaxed by it.
- **One canonical file.** Its identity was coordinated by the PR11 orchestrator for both sessions: this file, filed by
  B. The O14(b) session files nothing for it and cites this id. The check for an equivalent under another name, at
  `492325c4`, with its positive controls, found none (the follow-up B record's §9.28).
