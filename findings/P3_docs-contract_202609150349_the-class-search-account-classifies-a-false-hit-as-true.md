---
id: PR290-R3-THE-CLASS-SEARCH-ACCOUNT-CLASSIFIES-A-FALSE-HIT-AS-TRUE
severity: P3
disposition: deferred
category: docs-contract
pr: 290
reviewed_sha: 0320e2df9e7ca959f0a80146ef64126323839125
location: reviews/2026-09-14-o3-attribution-record.md:1071
provenance: introduced_by_feature
first_bad: 5da89afbc621b11317224019af0d7202b72e47d5
guard: the next change that edits `reviews/2026-09-14-o3-attribution-record.md` — it re-runs the class searches at the head it describes, saves the commands with their results, and classifies those results, not an earlier snapshot's
---

## Failure sequence

The record's §17 B5 entry (`reviews/2026-09-14-o3-attribution-record.md:1071`–`:1084`) cites its
"after" class search as run "at `0333e37c` after them
(`round2/b5-searches-after.txt`)" and says of it: "After the edits every remaining hit is one of: a
correction sentence quoting the old wording …, the search accounts themselves, or a true use
(… `unmodified` of the two corpus tests that are …)". The cited file
(`/home/ubuntu/o3-attribution-evidence/c3688ada0b9558fa35ba7b2ac9885261be7b887d/round2/b5-searches-after.txt:7`)
records, for the pattern `unmodified`, the hit "576:The invariants the contract names are held by
the existing assertions, unmodified:" — a sentence that was **false** at `0333e37c`, because round
2's B1 (`0de39a89`) had already modified the named test
`every_event_serializes_to_exactly_its_independently_written_payload` with the canonical pin; the
classification "a true use" does not hold of it. The operative sentence was corrected in the B5
commit itself (`5da89afb`; at `0320e2df` line 576 reads "unmodified but for the round-2 pin …",
`…/0320e2df…/residue/finding-2-3-commands.txt`), but the account still cites the pre-correction
snapshot as its measurement, gives patterns and abbreviated hit lists rather than the commands
and their results at the head it describes, and classifies a false hit as true (the round-3
fix-check lens's B5).

A successor reading the account concludes that the class searches were run on the final tree and
that every hit was examined and found true, when the cited snapshot predates the final edits and
contains a hit the classification does not cover.

## What the change that takes this up should do

Re-run the searches at the head the record describes, save each command with its complete result
under that head's evidence directory, and classify that result hit by hit in the record.
