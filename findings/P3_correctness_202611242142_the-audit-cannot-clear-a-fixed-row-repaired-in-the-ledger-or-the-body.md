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
guard: a pull request of its own for the body and ledger repair shapes, starting from ruling R73's proposal recorded below; until it merges, MAINTAINING.md step 5's clause "A fix confined to the pull request's own text needs no further pass", under which the enqueue may pass exactly this finding's two audit lines for the ids the body names as fixed in its text
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

`PR382-ACCEPTED-P3-REPURPOSES-FINDING-FILE`
(`findings/P2_correctness_202611172315_a-push-confined-to-the-ledger-can-unfile-an-open-finding.md`)
is the same ledger-only rule failing *open*: a ledger-only push keeps the review and can unfile an
open finding. This one fails *closed*.

## #433's withdrawn routes, and ruling R73's proposal

- #433 (a draft, parked at `9740f686`, not merged) carried two routes from its first reviewed head,
  `c0a81fd3`, that would have let the audit clear rows this finding names; `223c8670` withdrew both
  under ruling R73, and neither is on `master`.
- **The body route** (`a186f377`, with its validator exception `f2be3822`): a `fixed` row located at
  `pr.md:<line>` read `body-fixed:<id>`.
- **The witness lift:** a `fixed` row lifted `witnessed:<id>`.
- The body route's findings, as ruling R73 lists them: `PR433-LEDGER-BODY-LOCATION-MISMATCH`,
  `PR433-BODY-PATH-CASE-FOLD`, `PR433-R3-TRACKED-BODY-EVIDENCE`, `PR433-R3-NONFIXED-ROW-LOCATION`.
- The witness lift's findings: `PR433-MERGE-EDITS-CLEAR-WITNESS`,
  `PR433-INVALID-LEDGER-STATUS-TOKEN`, `PR433-ARCHIVE-LOOKUP-ERROR`,
  `PR433-R3-LEDGER-REWRITE-WITNESS`.
- Ruling R73's proposal, recorded as its proposal and not as an adopted design: "the body flag from
  the review alone, no validator edit, a witness lifted only by a repair outside the ledger, in a
  pull request of its own".

## What the change that takes this up should do

Both shapes go to that pull request. `PR382-ACCEPTED-P3-REPURPOSES-FINDING-FILE`'s change clears the
ledger shape only if a ledger rewrite satisfies a `fixed` row, as its file proposes (a rewrite there
reads as a repair); ruling R73 had #433's candidate refuse that. The choice is the owner's.
