---
id: R1F-RESOURCE-SNAPSHOT
severity: P3
disposition: deferred
category: docs-contract
pr: 321
reviewed_sha: 832e6ff09fcd0f9d5ba8c122b0234d1c3cd50abc
location: reviews/2026-09-25-gate-G5.md:59
provenance: fix_regression
first_bad: 3d2b7b8fad3e481a8fcb4be8ae2a68e05695ad48
guard: documented, not enforced: the repair lane's handover `g5_run6_r1.md` records the gap; taken up by a docs change that adds it to the report's deviations paragraph, or by the next gate run's runner writing the snapshot itself before every phase
---

## Failure sequence

Gate 5 run 6's repair brief required a build-box resource snapshot (`free -g` and target space)
before each phase. The four Linux ST-07 checks over the raw and filtered export copies
(`st07/adapter-exclusion/linux-linux-export{1,2}-{raw,filtered}.log`) all started at 11:39:05Z, with
their sampler at 11:38:51Z. The last snapshot before them is 11:34:57Z, taken before an ST-18 cut test of the preliminary v4
restored run (`st18-cuts/prelim-v4/restored/resources.log`). There is none immediately before the checks.

The repair lane's handover records the gap (`handovers/g5_run6_r1.md`, line 59, in the orchestrator's
working directory; it gives the checks as "at 11:37Z" and the nearest snapshot as "about 11:34Z",
where the log headers say 11:39:05Z and 11:34:57Z). The report's "Deviations from the run brief, stated"
paragraph (`:59`) does not. A reader of the report alone cannot learn that one phase ran without its
required snapshot.

**Location.** #321's row gives `:52`, the tree-table row naming the repair round's ST-07 checks. This
file locates `:59`, the deviations paragraph where the omission is.

**Provenance.** The checks are recorded in the report from `3d2b7b8f` (the `st07/adapter-exclusion/`
receipts are cited there), and the deviations paragraph has not named the gap at `3d2b7b8f` or at
`832e6ff0`. #321's row names `832e6ff0`, the reviewed head.

## Evidence

RECORD R1-6(a) of #321's final review of `832e6ff0`,
https://github.com/sourcemaps/upstroke/pull/321#issuecomment-5834621057 (full RECORD report:
https://github.com/sourcemaps/upstroke/pull/321#issuecomment-5834616927). The times above were read
for this filing from the log headers under `~/tactus-artifacts/g5-evidence-d724fb1/`. R1-6(b), the
body's limit field, was completed by root in #321's body and is not part of this finding. Neither gap
bears on any evidence. Deferred under the owner's round cap.

## What the change that takes this up should do

Add one sentence to the deviations paragraph: no snapshot immediately before the four Linux ST-07
checks at 11:39:05Z; the nearest is 11:34:57Z. Better, a runner that writes the snapshot itself
before each phase makes this a property of the tooling rather than a duty.
