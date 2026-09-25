---
id: R1F-SCOPE-WORDING
severity: P3
disposition: deferred
category: docs-contract
pr: 321
reviewed_sha: 832e6ff09fcd0f9d5ba8c122b0234d1c3cd50abc
location: reviews/2026-09-25-gate-G5.md:1063
provenance: fix_regression
first_bad: 3d2b7b8fad3e481a8fcb4be8ae2a68e05695ad48
guard: documented, not enforced: the clause grades and F60 carry the scope; taken up by a docs change to `reviews/2026-09-25-gate-G5.md` that adds "of each constructed finalization (F60)" in the places below, or carried as a lesson into the next gate report
---

## Failure sequence

Gate 5 run 6's report grades ST-18 on the repair round's kill cuts (**F60**). Their measured scope is
the 14 constructed finalizations: seven configurations × {Complete, Halted}. The graded rows state that
scope, "of each constructed finalization" (`:788`; also §4 group 5, §12 and §16 row 7). Four summary
places state finalization's kill convergence without it:

- `:1063`, the verdict: "finalization's convergence after a kill at every cleanup-effect boundary";
- `:981`–`:982`, §14 Q6: "Finalization converges to the same derived outcome after a kill at every
  cleanup-effect boundary";
- `:890`–`:891`, §14 Q2: "a process killed between any two of its cleanup effects";
- `:16`, the header: "every leftover kind at every occurrence position the step takes".

F60's label at `:137`, "process death at every cleanup-effect boundary of terminal finalization", is
the same shape. A reader of the verdict, Q2 or Q6 alone reads a universal claim where the evidence is
14 constructed shapes. In the same area, §12 (`:774`) lists "prepared and candidate-prepared pins
deleted" among what holds after each resume. No probe plant holds a candidate-prepared pin; §4 gives
those cells to the declared-pin kill test.

No grade is overstated: each grade carries the scope, and each sentence cites F60. All three final
lenses say the PASS stays true.

**Provenance.** `git blame` at `832e6ff0` puts `:16`, `:891`, `:981`, `:982` and `:1063` at
`3d2b7b8f`, the repair round's first commit. At that commit the grades already carried the scope, so
the mismatch dates from there. `:137` was rewritten at `832e6ff0`. #321's row names `832e6ff0`, the
reviewed head, as first bad; this file gives the measured commit.

## Evidence

RECORD R1-2, VERIFY FV-2 and REGRESSION P3-1 of #321's final review of `832e6ff0`, all P3 and all
PASS: https://github.com/sourcemaps/upstroke/pull/321#issuecomment-5834621057. The full RECORD and
VERIFY reports are https://github.com/sourcemaps/upstroke/pull/321#issuecomment-5834616927 and
https://github.com/sourcemaps/upstroke/pull/321#issuecomment-5834616639. Deferred under the owner's
round cap.

## What the change that takes this up should do

Add "of each constructed finalization (**F60**)" to the verdict, Q2, Q6, the header sentence and F60's
label. In §12, say that the candidate-prepared cells are the declared-pin kill test's. No figure
changes.
