---
id: R1F-TREE-TARGET
severity: P3
disposition: deferred
category: docs-contract
pr: 321
reviewed_sha: 832e6ff09fcd0f9d5ba8c122b0234d1c3cd50abc
location: reviews/2026-09-25-gate-G5.md:52
provenance: fix_regression
first_bad: 832e6ff09fcd0f9d5ba8c122b0234d1c3cd50abc
guard: documented, not enforced: the `st18-cuts` receipts' `base` fields name the base the counted run used; taken up by a docs change that gives the repair round's row both bases, or by the next gate report deriving its tree table from its receipts
---

## Failure sequence

The execution-tree table in Gate 5 run 6's report gives the repair round's tree,
`~/g5r6-work/r1-cut-tree`, one target base, `iso-g5r6r1-cut` (`:52`). The counted ST-18 phases behind
F60 and F61 ran on another:

- `positive`, `control-inert-cleanup` and `restored`, with their builds, all ran on
  `/home/ubuntu/disktarget/iso-g5r6r1-cut5`;
- that is the `base` field of each phase's `st18-cuts/<phase>/receipt.json` (read for this filing),
  each log header, and `st18-cuts/v5-staging-NOTE.md`;
- `iso-g5r6r1-cut` was the base of probe versions 2 to 4 and of the Linux ST-07 checks.

`832e6ff0` moved the counted run to the new base. Root's answer limited that commit to six
overclaiming sentences, so the table was not updated. That is why first bad is `832e6ff0`: at
`3d2b7b8f` the table matched the run it described.

A smaller instance of the same kind is at `:34`–`:35`, which says every "merge check, probe … and
gate attempt ran through `run.py`". The repair round's ST-07 checks, cut runs and gate attempts ran
through `run-r1.py`, as each log's first line shows; the next sentence does name `run-r1.py`.

Both bases are private bases behind `upstroke-build`, and every log shows
`CARGO_TARGET_DIR=(unset)`, so the header's rule-keeping claim holds. The table misattributes the
counted run. No grade is affected.

## Evidence

RECORD R1-4 and REGRESSION P3-2 of #321's final review of `832e6ff0`,
https://github.com/sourcemaps/upstroke/pull/321#issuecomment-5834621057 (full RECORD report:
https://github.com/sourcemaps/upstroke/pull/321#issuecomment-5834616927). Receipts are under
`~/tactus-artifacts/g5-evidence-d724fb1/st18-cuts/`. Deferred under the owner's round cap.

## What the change that takes this up should do

Give the repair round's row both bases, `iso-g5r6r1-cut` for probe versions 2 to 4 and the ST-07
checks and `iso-g5r6r1-cut5` for the counted ST-18 phases, and name `run-r1.py` in `:34`–`:35`. A
generated table, read from each receipt's `base`, would keep the two from drifting.
