---
id: PR290-R3-SEVEN-EXIT-CODE-CLAIMS-CITE-LOGS-WITHOUT-RECEIPTS
severity: P3
disposition: deferred
category: docs-contract
pr: 290
reviewed_sha: 0320e2df9e7ca959f0a80146ef64126323839125
location: reviews/2026-09-14-o3-attribution-record.md:678
provenance: introduced_by_feature
first_bad: fcfedc755f99c39a9178630dd70eb58352c9c268
guard: the next change that edits `reviews/2026-09-14-o3-attribution-record.md` — it cites a saved receipt for every exit status it states, or states only what the cited log shows
---

## Failure sequence

Seven sentences of the record state an exit status and cite a log that carries none, re-executed
at `0320e2df9e7ca959f0a80146ef64126323839125` and saved in
`/home/ubuntu/o3-attribution-evidence/0320e2df9e7ca959f0a80146ef64126323839125/residue/finding-4-commands.txt`
(the round-3 record lens's finding 2):

- `reviews/2026-09-14-o3-attribution-record.md:677`–`:679` says of the Phase 6 body draft:
  "`phase6/draft-body-for-validation.md` through `validate-pr-body.sh`, exit `0`, and
  `validate-pr-ledger-evidence.sh <head>` with the finding committed on a throwaway commit, exit
  `0`: `phase6/validate-pr-body.log`, `phase6/validate-pr-ledger-evidence.log`". The two cited
  files, under
  `/home/ubuntu/o3-attribution-evidence/8b28944f9607447448f5d9b9950ee49596073e58/phase6/`, are
  both **0 bytes**: the two validators print nothing on success, and the session captured their
  exit status only on its own console, not in the file.
- Five more "exit `0`" claims cite logs, under the same base directory, that hold a success banner
  and no captured return code: `:622` (`phase4/test-internals-notes.log`, 115 bytes, last line
  `internals notes fixtures: 41 cases passed`), `:624` (`phase4/test-docs-consistency.log`, 41
  bytes, `documentation consistency fixtures: PASS`), `:655` (`phase5/test-docs-consistency.log`,
  the same 41 bytes), `:681` (`phase6/test-pr-policy.log`, 123 bytes, `PR policy fixtures passed`)
  and `:682` (`phase6/test-pr-ledger-evidence.log`, 35 bytes, `PR ledger evidence fixtures
  passed`); none contains an `rc=` line.

The same capture gap exists for the final head's two validator logs
(`…/0320e2df…/pr/validate-pr-body.log` and `validate-pr-ledger-evidence.log`, both 0 bytes); the
two validators were re-executed at `0320e2df` with the exit status captured this time —
`…/0320e2df…/pr/validate-pr-body-with-rc.log` and `validate-pr-ledger-evidence-with-rc.log`, each
`rc=0` — and the final head's ten-gate receipts (`…/0320e2df…/gates/w1-eight-iso-attempt1.log`,
`w1-eight-iso rc=0`) and CI run 34924988456 establish the final head's success independently. The
historical claims are not recoverable from the artifacts they cite.

A successor reading the record concludes that each cited log carries the exit status the sentence
states, and finds an empty file or a banner with no receipt.

## What the change that takes this up should do

For each of the seven sentences, cite a saved receipt — the re-executed validator logs with `rc=`
for the two Phase 6 claims, and for the five gate claims either re-executed logs that capture the
exit status or the sentence narrowed to the banner the log shows.
