---
id: PR258-FINAL-CHECKS-COVERAGE-MISATTRIBUTED
severity: P3
disposition: deferred
category: docs-contract
pr: 258
reviewed_sha: f6fd7fc0faa7e79618da922270ebe666f6100a5f
location: 48f2af7a:reviews/2026-09-09-g4-fix-temporary-object-fanout-record.md:1273
provenance: introduced_by_feature
first_bad: f6fd7fc0faa7e79618da922270ebe666f6100a5f — §14.3 is new in that commit
guard: the next edit of the #258 body takes it up — its Gates paragraph says what the checker checked (the round-7 files cited before it ran) and that the four written after it are attested by the manifest, not by the checker; the four files exist and carry manifest rows, so no evidence is missing
---

## Failure sequence

Record §14.3 says the checks "that each round-7 file cited here and in the body exists — are
`45-r7-final-checks.log`", and the body's Gates paragraph says that log "covers ... the cited
round-7 files".

    45-r7-final-checks.py, line 46, collects every `4N-r7-*` name the record and the body cite
    -> line 47 lists `45-r7-final-checks.log`, `46-r7-pr-body.md`, `47-r7-final.patch` and
       `48-r7-manifest-update.py` as `later`, and the loop `continue`s past each of them
       ("produced after this check, by construction")
    -> the existence assertion runs for the six other cited names only — the log's `CITED=`
       lines — and then reports `CITED_FILES_PRESENT=PASS`
    -> the review's reasoned sequence: delete `46-r7-pr-body.md` and rerun; the cited-file
       phase still passes

Reasoned from the checker's source, as the review says; the deletion was not executed. The four
files exist and carry rows in `19-r3-evidence-manifest.md` (written by `48-r7-manifest-update.py`),
and the review's own read-only recount "found 229 files, 228 rows, only the manifest omitted,
with no digest or size mismatch" — so the evidence is present and what is wrong is the sentence
that attributes its verification to a checker that skipped it by name: false attribution, not
missing evidence. Finding 5 of the `ultra` review of `f6fd7fc0`, posted on the pull request as
the SHA-bound comment.

## Since 2026-11-24

The record left master on 2026-11-24 (`bde77050`) and is read from history at the location above;
its own copy, the §14.3 sentence there, is archived evidence that nobody keeps true and is not
corrected. What stays open is the #258 body's Gates paragraph, which in the body as saved on
2026-11-24 (`~/selfhost-docs5-20261124/evidence/r2-bodies/pr258-body.md`, line 38) still reads as
the failure sequence quotes it: "`45-r7-final-checks.log` covers whitespace, the freeze, the local
body validation and the cited round-7 files".

## Why it is deferred

Reasoned-only, no evidence missing, no failing test or witness; bears on no Gate 4 pass-rule
clause and no row-10 evidence, so the owner's rule files it and merges.

## What the change that takes this up should do

Make the #258 body's Gates sentence say what ran: the checker verified the cited round-7 files
that existed before it ran, skipped by name the four it knew would be written after it, and those
four are attested by the manifest's rows. Or run a checker over the full cited set after the last
file is written and cite that log in the body instead.
