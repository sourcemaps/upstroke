---
id: PR290-R3-THE-ROUND-2-ACCOUNT-MISSTATES-ITS-FIGURES
severity: P3
disposition: deferred
category: docs-contract
pr: 290
reviewed_sha: 0320e2df9e7ca959f0a80146ef64126323839125
location: reviews/2026-09-14-o3-attribution-record.md:141
provenance: introduced_by_feature
first_bad: 703bcd0794365d65fde5a1bb05aa4dbe93f4dfd6
guard: the next change that edits `reviews/2026-09-14-o3-attribution-record.md` or the pull request #290 body — it corrects each sentence below to the saved output of its own command and cites that output by path, the way this pull request's round-1 and round-2 entries cite theirs
---

## Failure sequence

Four sentences of the round-2 account state a figure their own command or file contradicts, each
re-executed at `0320e2df9e7ca959f0a80146ef64126323839125` with the output saved in
`/home/ubuntu/o3-attribution-evidence/0320e2df9e7ca959f0a80146ef64126323839125/residue/finding-1-commands.txt`
(the round-3 fix-check lens's B3 and the record lens's finding 1):

- The body (`…/0320e2df…/pr/body.md:257`, `pr.md:259` in the review's copy) says this head's `src`
  tree "differs from it by round 1's B1, B2 and B4 (`git diff --stat fcfedc75 HEAD -- src`: four
  files) and by round 2's pin". The command prints **five** files — `src/engine/tests.rs`,
  `src/events/log/tests.rs`, `src/events/mod.rs`, `src/interaction.rs`, `src/topology/events.rs`
  — `5 files changed, 120 insertions(+), 17 deletions(-)`; the fifth is round 2's canonical pin, and
  "four" describes the range ending at `c3688ada`.
- The body (`body.md:176`, `pr.md:178`) says the 25 commits are "the first landing's ten, round 1's
  nine, and round 2's seven". `git rev-list --count c3688ada..0320e2df` prints **6**
  (`0de39a89`, `eb1c1d1c`, `703bcd07`, `0333e37c`, `5da89afb`, `0320e2df`); the total of 25 is
  right, the breakdown is not.
- The record (`reviews/2026-09-14-o3-attribution-record.md:141`) says "the seven enum-variant
  constructions above are the two legacy emitters and six fixture construction sites". Two plus six
  is eight, and eight is what the base census lists: **eight** value-position constructions of the
  record, two emitters and six fixture sites. Seven is the count of `EventBody::DesignDefect {` and
  `TopologyEventBody::DesignDefect {` constructions — the two emitters and five of the six fixture
  sites — because the sixth fixture site, the canonical `serde_json::to_value(DesignDefect { .. })`
  at base `src/topology/events.rs:4361`, constructs the record and no enum variant. The sentence
  puts the record count's parts under the enum-variant count's total.
- The record (`reviews/2026-09-14-o3-attribution-record.md:1088`) says of the commits between the
  B1 commit `0de39a89` and the code head `5da89afb` that "the commits between are this record's";
  `git show --stat` shows `eb1c1d1c` also changes the F1 finding file, `703bcd07` also changes the
  O3 finding file, and `0333e37c` changes `docs/internals/events/mod.md` and
  `docs/internals/interaction.md`. The body (`body.md:249`, `pr.md:251`) says those commits "are
  the record and the two findings' corrections" and omits the two notes.

A successor reading the account concludes that round 2 touched one more commit and one fewer
source file than it did, that the base held seven constructions of the record, and that the
round's docs commits touched only the record — and, following the cited commands, finds each
figure contradicted by the command's own output.

## What the change that takes this up should do

Replace each figure with its command's saved output — five files, six commits, eight record
constructions of which seven construct an enum variant, and the commit list with every file each
commit touches — and cite that output by path.
