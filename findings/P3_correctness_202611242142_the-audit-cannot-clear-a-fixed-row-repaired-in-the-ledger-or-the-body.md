---
id: AUDIT-FIXED-ROW-NEEDS-A-PUSH-OUTSIDE-THE-LEDGER
severity: P3
disposition: deferred
category: correctness
pr: 415
reviewed_sha: c5061b817c9e61993bf19cf43a06cf81720bdefe
location: scripts/pr-ready-audit.sh:1400
provenance: pre_existing
first_bad: 112c4aff40c0f075348947dbb2541e79f2d3c60e
guard: master's interim clause in MAINTAINING.md step 5, "A fix confined to the pull request's own text needs no further pass", with its personal read and narrowly named enqueue exceptions
---

# The readiness audit cannot clear a fixed row whose repair is in the ledger or in the body

## Failure sequence

`scripts/pr-ready-audit.sh` is the same at `c5061b81` and at `master` `5cda2c86`; the line
numbers below read the same at both.

**1. A `fixed` row whose repair is in `findings/` or in the body.**

    a pull request's finding is repaired in `findings/` (a `findings/` pull request repairing its own findings) or in the pull request body, and its ledger row records `fixed`
    -> the audit counts every `findings/` path as ledger (`scripts/pr-ready-audit.sh:1218`), so a `findings/` repair reads `moved=ledger-only` (`:1222`)
    -> a body repair is no commit, so the head does not move at all
    -> either way, a `fixed` row needs `moved=repairs` or `merge-edits:*` (`:1399-1400`)
    -> the audit blocks the row as `fixed-but-head-unmoved:<id>`

**2. A witnessed finding fixed in the body.**

    a finding whose verdict carries a `witness` key is repaired by a body correction, and its ledger row records `fixed`
    -> the audit raises `witnessed:<id>` from the verdict's `witness` key (`:1328-1330`) before it reads the ledger row
    -> it reads only the newest frontier verdict (`:688-695`)
    -> the finding keeps its `witnessed:` blocker until another frontier pass on a new verdict

In both shapes the repair is real and the audit cannot print READY. The honest state is closer to
NEEDS-ATTEST; that is ruling r54's reading.

## The sightings

Two audit lines, each printed by the operator's settlement with
`UPSTROKE_REVIEW_AUTHOR=eventloops bash scripts/pr-ready-audit.sh <N>` on 2026-11-24.

- **The first shape: #415 at `fd7c7324`.**

      #415  findings       fd7c732  NOT-READY     verdict=CHANGES_REQUIRED reviewed=b921e4a moved=ledger-only blockers=draft,fixed-but-head-unmoved:PR415-SUMMARY-RATE-POPULATION-STALE,fixed-but-head-unmoved:PR415-SUMMARY-MASTER-RERUN-STALE,fixed-but-head-unmoved:PR415-FIRST-FINGERPRINT-NOT-WORKTREE-ADD

  Of #415's four round-two findings, two were fixed in `findings/` by `30733b93` and two in its
  body. Ruling r54 enqueued #415 past these blockers, and it merged as `5cda2c86`.
- **The second shape: #413 at `5d6688f6`.**

      #413  docs           5d6688f  NOT-READY     verdict=CHANGES_REQUIRED reviewed=5d6688f blockers=draft,witnessed:PR413-SCOPE-OMITS-HEAD-COMMIT

  The finding is a body sentence (Scope counted four commits where the range holds five). Its
  witness is `git rev-list --count 48f2af7a..5d6688f6`, which prints 5. It was fixed in the body,
  and ruling r57 settles #413 past this blocker.
- Ruling r52 step 6 likewise allowed #411 to be enqueued with a `witnessed:` blocker.

## Why P3

The audit fails closed, so the cost is a ruling or a review pass, never a wrong merge.

## Related, not the same

`PR382-ACCEPTED-P3-REPURPOSES-FINDING-FILE`, recorded at
`3b04c1d7:findings/P2_correctness_202611172315_a-push-confined-to-the-ledger-can-unfile-an-open-finding.md`,
is the old ledger-only rule failing *open*: a ledger-only push kept the review and could unfile an
open finding. This one fails *closed*. Restricting review retention to additions does not repair
this finding: a ledger rewrite satisfies no `fixed` row and lifts no witness.

## Current guard

This P3 remains open with disposition `deferred`. Master's interim clause in `MAINTAINING.md`
step 5, already present at `3b04c1d7`, governs a fix confined to the pull request's own text. On
that clause's terms, corrected facts and any witness are run again and recorded in the body, and
the owner or delegated agent records the word-diff read. The clause permits enqueueing past
exactly `fixed-but-head-unmoved:<id>` and `witnessed:<id>` for exactly the ids the body names as
fixed in its text, and no other blocker. It remains the guard while the audit support is deferred.

## Design note only: withdrawn work and r73

The withdrawn implementation began at `a186f377`: bit 4 of the review parser's finding flags
identified an exact `pr.md:<line>` body location, and a `fixed` row could produce
`body-fixed:<id>`, NEEDS-ATTEST. It also let a `fixed` row lift a witness for a repair push or a
body correction; initially `merge-edits:*` qualified too, until `9f9e7904` removed that route.
The later version at `63e33659` required matching ID, severity and body-location category in the
review and row, with P3 eligible in every lane and P2 only in the findings lane. The validator's
virtual `pr.md` location exception came from `f2be3822`.

These body flags, body-row handling, validator exception and witness-clearing paths were
withdrawn by `c2ec6bbe`. The current audit blocks a witnessed finding before consulting its row;
neither a body correction nor a ledger rewrite lifts it. The file was restored under r73 by
`4c372d54`; the earlier closure at `c0a81fd3` does not resolve this deferred work.

The prior finding IDs remain part of that history:

- `AUDIT-FIXED-ROW-NEEDS-A-PUSH-OUTSIDE-THE-LEDGER` is this finding;
  `PR382-ACCEPTED-P3-REPURPOSES-FINDING-FILE` is the separate review-retention defect above.
- `PR433-ARCHIVED-LEDGER-KEEPS-REVIEW` and `PR433-MALFORMED-LEDGER-STATUS` are named in
  `9f9e7904`'s audit and fixture changes. `0acecb8b` carries
  `PR433-STATUS-RECORD-COVERAGE`, `PR433-INVALID-LEDGER-STATUS-TOKEN` and
  `PR433-ARCHIVE-LOOKUP-ERROR` in its finding trailers.
- `63e33659` carries `PR433-BODY-PATH-CASE-FOLD`,
  `PR433-LEDGER-BODY-LOCATION-MISMATCH` and `PR433-BODY-P2-LANE-ELIGIBILITY`;
  `3d2f9045` carries `PR433-ARCHIVE-EXEMPTION-WORDING` and
  `PR433-LEDGER-VIRTUAL-LOCATION-WORDING`.

R73 proposes a separate future pull request: the review would be the sole source of the body
flag, the validator would remain unchanged, and only a repair outside the ledger could lift a
witness. This is a design note only, not an implemented exception or a change to the current
guard.

## What the change that takes this up should do

Take up the body and ledger repair shapes in a separate pull request, using r73's proposal above
as a design note. Until that work lands, master's interim clause remains in force and this P3
stays open.
