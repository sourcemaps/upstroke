---
id: PR399-FLOOR-PROSE-NAMES-SPLIT-TESTS
severity: P3
disposition: deferred
category: docs-contract
pr: 399
reviewed_sha: 14cb317f32d44f7904f80fb45080d47386228d6b
location: MAINTAINING.md:78
provenance: introduced_by_feature
first_bad:
guard: the shard-packing change, which rewrites the Windows floor prose in .github/workflows/ci.yml, MAINTAINING.md step 3, DESIGN.md's 2026-09-01 row and docs/internals/effects/tests/ci_model.md
---

## Failure sequence

1. A maintainer looks into a slow Windows shard. They read in `MAINTAINING.md` step 3 (or in
   one of the three other places listed below) that two single tests set the Windows floor,
   and they copy one of the two names given there.
2. They run that one test on its own, the usual way to time or reproduce a single test:
   `cargo test … -- --exact engine::topology::recover::tests::a_kill_at_every_cell_of_a_complete_finalization_converges_on_the_next_resume`.
3. That function no longer exists. Pull request #399 replaced it with parts whose names
   extend the old one, and `--exact` matches whole names only. The filter selects nothing:
   the harness reports `0 passed`, filters out every test and exits 0.
4. The maintainer sees a green, fast run and concludes that the test is quick, or that it
   passes on their machine. Neither conclusion is about the test the prose meant. The second
   named test, `workspace_manager::tests::r_o2_a_cross_kind_scrub_stopped_at_any_phase_converges_with_no_instance_registered`,
   fails the same way.

Nothing fails. The documents name tests that this head does not have, and the only sign is a
count of zero that no one asked to check.

## The facts, at 14cb317f

- Four places say that two single tests set the floor under the Windows prediction, about
  9 minutes of their leg's harness each, and name both tests:
  - `MAINTAINING.md` step 3, :78-83;
  - `docs/internals/effects/tests/ci_model.md`, :842-849;
  - `DESIGN.md:96`, the 2026-09-01 self-hosted Windows test leg row;
  - the comment in `.github/workflows/ci.yml` above the hosted Windows shard matrix, :669-675.
- The two names are
  `engine::topology::recover::tests::a_kill_at_every_cell_of_a_complete_finalization_converges_on_the_next_resume`
  and
  `workspace_manager::tests::r_o2_a_cross_kind_scrub_stopped_at_any_phase_converges_with_no_instance_registered`.
- This pull request removes both tests. It replaces each one with parts whose names extend
  the original name.
- A substring filter on the old name, which is how the shard filters and a plain
  `cargo test <name>` match, still selects every part. So the shard rule and the claim that
  every test runs in exactly one leg are unaffected. Only `--exact` with the old full name
  breaks. It selects nothing and exits 0.
- #392 wrote the prose from run 44736631711's measurement. "Neither changed by the re-split"
  was true of the 2026-11-20 re-split, which moved filters and not tests. This pull request
  is the first change that moves the tests themselves.

## Why it is deferred

This pull request does not edit `.github/`, `MAINTAINING.md` or `DESIGN.md`. Changes there
are the owner's to read, and a split that only touches test code should not pull them into
that read. The prose is also a measurement, not only a list of names. Swapping in the part
names would claim that each part sets a floor of about 9 minutes, and no run has measured
that. The split exists to lower that floor. The next change to this prose is the queued
shard-packing change, which rewrites it from new measurements. It is the right place to
correct the names and the numbers together. Until then the stale names mislead only a reader
who copies them into `--exact`. Both still work as substring filters, so P3.

## What the change that takes this up should do

The shard-packing change should restate the floor in all four places from its own
measurement, and identify the slowest tests in a way that holds up under a later split:

- name the parts (the full names of the tests that exist at its head), or
- name the filter that selects them. Each part's name extends the old one, so the old full
  name works as that filter. The prose has to say it is a substring filter and not an
  `--exact` name;

and give the measured time of each part, or the slowest part, from that change's CI run. If
the split brings the longest single test down far enough that single tests no longer set the
floor, the change should say so and drop the paragraph. When the prose is corrected, delete
this file.
