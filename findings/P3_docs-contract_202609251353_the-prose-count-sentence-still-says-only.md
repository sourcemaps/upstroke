---
id: R1F-PROSE-COUNTS
severity: P3
disposition: deferred
category: docs-contract
pr: 321
reviewed_sha: 832e6ff09fcd0f9d5ba8c122b0234d1c3cd50abc
location: reviews/2026-09-25-gate-G5.md:23
provenance: fix_regression
first_bad: 3d2b7b8fad3e481a8fcb4be8ae2a68e05695ad48
guard: documented, not enforced: `figures/figures.py` derives every count a grade rests on, so the header's first sentence holds; taken up by a docs change that names the remaining typed counts in §0's closing note or softens "only" and "everything else"
---

## Failure sequence

The header of Gate 5 run 6's report (`:21`–`:24`) says the prose states counts other than figures "in
two ways only": tables and lists a generator printed, and "the few counts §0's closing note names".
The closing note (`:157`) ends: "Everything else numeric in the prose is an identifier — a row,
section, line, pull-request, run or job number, or a hash." That wording was written to narrow RECORD
P3-1 of the first review, and it is still broader than true. RECORD's scan of the report's section
sources found typed counts that are neither generator output nor named in the note:

- "Four full runs" (line 220);
- "the 29-pair pin" (383);
- "Counted from the definition: **seven**" (393);
- "run 4's six recipe sets … and one set of this run's own" (417);
- "with `--exact` in five of the seven sets" (422);
- "The five merges since" (478);
- "two of the Windows-only points, recording three point-and-mode coordinates" (549);
- "every length 13 bytes longer" (701);
- "three files added and none of run 4's removed" (795);
- "eight P1s" (1051, 1075).

REGRESSION's P3-3 adds "the same 168 entries" (194) and "the 168 `executed` entries" (560). Every
count is correct, and no grade rests on one, so the header's first sentence ("Every count a grade
rests on is a figure") holds. Only the absolute "only" / "everything else" overstates.

**Location.** #321's row gives `reviews/2026-09-25-gate-G5.md:39`. At `832e6ff0` that line is the
`SigIgn` paragraph. The "two ways only" sentence is `:23`, the line RECORD R1-3 cites, and its
companion is `:157`; this file locates `:23`.

**Provenance.** `:23` and `:157` blame to `3d2b7b8f`, where the sentence and the typed counts
(for example "The five merges since", "eight P1s") both already stand. #321's row names `832e6ff0`,
the reviewed head.

## Evidence

RECORD R1-3 and REGRESSION P3-3 of #321's final review of `832e6ff0`,
https://github.com/sourcemaps/upstroke/pull/321#issuecomment-5834621057 (full RECORD report:
https://github.com/sourcemaps/upstroke/pull/321#issuecomment-5834616927). RECORD checked each count
against its source. Deferred under the owner's round cap.

## What the change that takes this up should do

Name these counts, each with the file it was checked against, in §0's closing note. Otherwise, soften
"in two ways only" and "Everything else numeric" to what the report actually does. Making the counts
figures is the stronger form.
