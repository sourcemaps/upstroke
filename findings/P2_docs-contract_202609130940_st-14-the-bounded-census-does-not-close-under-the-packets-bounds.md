---
id: PR10-ST14-BOUNDED-CENSUS-DOES-NOT-CLOSE
severity: P2
disposition: deferred     # the correction is the packet's, and the packet is the owner's
category: docs-contract
pr: 279
reviewed_sha: 66fdf5fbe2ccfa41dec66e221c714cc257f09022
location: src/topology/census.rs:2472
provenance: pre_existing   # ST-14's sentence and G5's clause predate the slice; the slice is the first to measure them
first_bad:
guard: the project owner — the packet correction to `decisions.bounded_census`, ST-14 and G5 (escalated by the PR10 orchestrator, 2026-09-13)
---

## Failure sequence

ST-14 says "the census explorer completes for the stated bounds", and Gate 5 passes "iff bounded
census complete with totality asserted and every plan_transition arm covered". The tree's census
(`src/topology/census.rs`) explores breadth-first under the packet's bounds
(`decisions.bounded_census.bounds`: three originals, two repairs in two lineages, two generations
per task, two attempts per generation, four integration sequences, verification defers under the
fixture's `max_defers` of two, two open questions, one review pass, two resumes), keyed on the
fold's state, which is the packet's own abstraction (`decisions.bounded_census.abstraction`) up to
four residues nothing reads; it stops at its state ceiling of 20,000 states and asserts
`truncated` (`the_census_runs_at_the_packets_bounds_and_says_where_it_stopped`,
`a_census_that_hits_its_ceiling_says_so`), which is what the packet's own `bounded_census.claim`
line describes: "bounded evidence … unbounded closure is not claimed".

**The space under the packet's bounds does not close, under the packet's own abstraction.**
Measured, on the build box, at `66fdf5fb`:

- The repair session's curve (`census-curve.log`): the shared census at 20,000, 50,000, 100,000
  and 200,000 states costs 14, 38, 80 and 159 seconds and 1.2, 3.0, 6.2 and 12.6 GB — linear in
  states — while the distinct states per trace depth double at every level from depth 12 to 17
  (1,861; 3,627; 7,427; 15,636; 33,453; 71,535), 200,000 states reaching depth 18.
- The orchestrator's research (`research_census_1`, 2026-09-13): projecting every explored state
  onto the packet's quotient — SHAs and refs relabelled, paths as regions, only the predicates and
  classes `plan_transition` reads retained — merges 1.0–1.1 concrete states into one abstract state
  at every depth (`probeA-20k-L4.log`, `probeB-debug-L4-300k.log`), and the quotient is sound for
  the generator (0 disagreements among 1,722 abstract classes with two or more members). The
  abstract states per depth grow ×2.15, ×2.14, ×2.11 and ×2.08 at depths 17, 18, 19 and 20
  (140,388; 296,529; 615,554 states at depths 18–20; `deep-release-L4-dfs.log`), the concrete
  space's own rate. A depth-first continuation from the complete depth-20 frontier — a release
  build, 16 threads, a hash-only visited set — reached **50,661,094 distinct abstract states in its
  1,500-second budget without closing** (16,695,460,640 offers, 11.6 GB), having expanded 41 of its
  615,554 depth-20 seeds; that is a floor on the space, not an estimate, and the growth factor's
  decline (0.02 per level) is a tenth of what a space that closes shows. The trace ceiling of 48
  did not bind at any depth reached.
- Tightened bound sets do close — every dimension at its minimum at 1,986 abstract states
  (`r2-allmin-L4-dfs.log`); generations per task 1 with open questions 0 at 6,949,760 states in
  152 seconds and 5.0 GB (`r2b-gen1-q0-L4-dfs.log`); generations 1 alone and questions 0 alone do
  not close within about 480 seconds (`r2c-gen1-L4-dfs.log`, `r2d-q0-L4-dfs.log`) — but those
  rewrite the packet's `bounds` line, which the slice may not do.

So no run of the explorer can establish the Gate 5 clause at the packet's bounds: "completes" is
unsatisfiable as written. The logs: `~/pr10-evidence/66fdf5fbe2ccfa41dec66e221c714cc257f09022/census-research/`
on the build box (copied from `~/orch-pr10/research/census-1/`, beside the orchestrator's answer
`~/orch-pr10/answers/repair_279_r2-1.md` and its escalation `~/orch-pr10/ESCALATION.md`), and
`~/pr10-evidence/r2/census-curve.log`; the record is `reviews/2026-09-12-pr10-record.md` §3 R16,
§8, §11 and §13.

## What the change that takes this up should do

Owner, as the ledger records it: the project owner — the packet is the owner's, and the G4 row-10
amendment is the precedent for correcting it after a measurement.

Choose the correction to `decisions.bounded_census` (and ST-14's sentence, and G5's clause), each
with its measured size under the packet's abstraction:

1. **A trace-length bound stated as a bound** ("every trace ≤ T events"): the census then closes the
   depth-≤-T prefix exactly, and totality, arm coverage and replay hold over it. Closed sizes:
   T = 14, 13,937 states (about 10 s and 0.9 GB in the present representation); T = 15, 28,075;
   T = 16, 58,522; T = 17, 124,035 (about 90 s, 7.8 GB); T = 19, 560,952; T = 20, 1,176,506
   (about 14 min and 74 GB as the census is built today). The bound would sit far below the depth
   at which the packet's dimensions are jointly reached, so the fourth sequence, the second repair
   and the second lineage stay the seeded deep census's.
2. **The state ceiling stated as the bound** ("the explorer explores the first N states
   breadth-first and reports truncation"): what the tree does today (N = 20,000, `truncated`
   asserted). The smallest textual change; "completes" then means "completes its bounded prefix".
3. **A per-dimension product**: ST-14 satisfied by a family of censuses — the breadth-first prefix
   plus one seeded census per dimension the prefix does not reach, each closing under its own
   ceiling. The tree has the shape: `deep_census()` closes at 25 states from a 15-event seed
   (`probe-deep-census.log`) and reaches the fourth sequence, the second repair and the second
   lineage (`every_declared_dimension_is_reached_at_its_bound`).
4. **Tightened dimensions** (a change to the `bounds` line): every dimension at its minimum closes
   at 1,986 states; generations per task 1 with open questions 0 closes at 6,949,760 (release
   build, 16 threads, 152 s; by arithmetic about 83 minutes and 438 GB in the present debug-profile
   representation, so an ignored, lean, release-profile census or nothing). The cost is coverage: a
   second generation per task is the retained-generation recovery path (ST-11) and the questions
   are the park-and-answer path (ST-12), which a closed space without them no longer witnesses.

Under 1 and 2 the `truncated`-asserting tests stand as they are; 3 rewrites ST-14's sentence to a
family of censuses, each closing; 4 rewrites the `bounds` line and re-points
`every_declared_dimension_is_reached_at_its_bound` at the new bounds. Until the owner chooses, the
Gate 5 clause cannot be established, the census key is not worth changing (the four residues are
worth 9 per cent of the states), and the slice claims what the tree does: a truncated prefix that
says so, every declared bound reached by equality, every arm executed, the classifier equal live
and on replay over the explored set.
