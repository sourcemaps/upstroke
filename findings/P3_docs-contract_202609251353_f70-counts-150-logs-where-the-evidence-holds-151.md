---
id: R1F-F70-COUNT
severity: P3
disposition: deferred
category: docs-contract
pr: 321
reviewed_sha: 832e6ff09fcd0f9d5ba8c122b0234d1c3cd50abc
location: reviews/2026-09-25-gate-G5.md:147
provenance: fix_regression
first_bad: 832e6ff09fcd0f9d5ba8c122b0234d1c3cd50abc
guard: documented, not enforced: the head's gate log is in the evidence manifest and `figures/figures.py` regenerates F70 as 151; taken up by a docs change that states F70's cutoff or regenerates it after the last log, or by the next gate report generating its figures after its own final log
---

## Failure sequence

F70 (`:147`) counts the `SigIgn` receipts of the repair round's runner: "across this round's 150
run-r1.py logs in the evidence: 0000000000000000 in 150". Its derivation is every `run-r1.py` header line under the evidence
directory. The sequence:

1. The figures were generated at 12:42:36Z (`figures/figures.json`).
2. The report was committed as `832e6ff0`.
3. The head's own gate attempt then wrote `gates/head-attempt-832e6ff.log` at 12:46:40Z, the 151st
   such log.
4. The manifest froze the directory with 151 logs, so `figures/figures.py` over the frozen evidence
   gives "151 … in 151". The report says 150.

The substantive claim holds at 151 of 151: every command the round's runner launched ran with mask
`0x0`. What fails is the header's promise that each figure is generated from the saved files named
beside it, for this one figure. RECORD found 72 of 73 figures byte-identical on regeneration; F70 was
the one difference.

**Provenance.** F70's row was rewritten at `832e6ff0` (`git blame`), and that commit's own gate log is
the 151st.

## Evidence

RECORD R1-1, VERIFY FV-1 and REGRESSION's observation O-1 of #321's final review of `832e6ff0`,
https://github.com/sourcemaps/upstroke/pull/321#issuecomment-5834621057 (full RECORD report:
https://github.com/sourcemaps/upstroke/pull/321#issuecomment-5834616927). The file times are from
`~/tactus-artifacts/g5-evidence-d724fb1/`. The repair lane's `final-state-r1.md` and its handover
already noted the 151st log. Deferred under the owner's round cap.

## What the change that takes this up should do

State F70's cutoff ("as of the figures' generation, before the head's own gate attempt"), or
regenerate the figures after the last log is written. A gate report's generator should run after
every evidence file it counts has been written: that is the ordering this finding records.
