---
id: PR441-CONTRACT-NOTE-TEST-SCANS-WHOLE-DOCUMENT
severity: P3
disposition: deferred
category: correctness
pr: 441
reviewed_sha: dc7af793fb624aa58d69b29ea3890a083a05b943
location: src/runner/host/tests.rs:4341
provenance: fix_regression
first_bad: 99c4a2da
guard: the next change to `the_runner_contract_note_counts_the_subscription_overlay_among_production_values`, which confines it to the overlay paragraph
---

# The contract-note test searches the whole note for its retired phrases

Filed deferred from #441's review of the reviewed commit. CI only: the test reads a note and nothing else.

## Failure sequence

    append an unrelated sentence containing `production's only value` to the runner note
    -> `the_runner_contract_note_counts_the_subscription_overlay_among_production_values`
       collapses the whitespace of the whole of `docs/internals/runner/contract.md` (line 4340)
       and refuses each retired phrase anywhere in it (lines 4341 to 4343)
    -> the test exits 101, although the overlay paragraph it claims to check is unchanged

The overlay paragraph is `docs/internals/runner/contract.md:1233` to `:1244`. The test's pins are searched over the same whole note (lines 4353 and 4354), while its message says "the runner-contract note's overlay paragraph must name {pin}" (line 4355). The mirrored note, `docs/internals/runner/host/tests.md:1614`, says the test "requires the paragraph to name" them.

## Reproduction

The reviewer's: appending an unrelated sentence containing `production's only value` to the runner note made the test exit 101.

## What the change that takes this up should do

Confine the test's searches to the overlay paragraph, so that the retired phrases are refused, and the pins required, there and not across the whole note. Keep the mirrored note true to the code. The regression test for it is named for what it shows, fails at the reviewed commit and passes after the change.
