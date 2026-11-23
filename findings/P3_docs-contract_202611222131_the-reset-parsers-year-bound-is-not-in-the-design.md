---
id: PR402-RESET-YEAR-BOUND-NOT-IN-DESIGN
severity: P3
disposition: accepted-risk
category: docs-contract
pr: 402
reviewed_sha: 61c0f6e48990247f1a13a3f530ede781924a9c25
location: src/agent/adapter.rs:301
provenance: introduced_by_feature
first_bad: 6c18c125, #402's repair of PR380-RESET-PARSER-NON-RFC3339-YEAR
guard: the next change to design/13_design_capacity_engine.md's rate-limit paragraph or to the reset parser
---

## Failure sequence

From #402 round three's review:

    a well-formed `resets 5:30am (UTC)` whose nearest occurrence is outside 0000 to 9999
    -> `reset_at_from_limit_message` returns `None`, so no reset is recorded
    -> DESIGN §13 says a valid UTC reset selects its nearest occurrence, and names no year bound
    -> the bound is recorded only in the internal notes

The lines, at `src/agent/adapter.rs:301` to `:304`, added by 6c18c125:

```rust
let (year, _, _) = crate::util::civil_from_days(nearest.div_euclid(86_400));
if !(0..=9_999).contains(&year) {
    return None;
}
```

DESIGN §13's rate-limit paragraph (`design/13_design_capacity_engine.md:15`) says: "The messages
give no year, and the short form no date, so the reset is the occurrence nearest the moment the text
is read". Where it lists the readings that record no reset, it names "a zone other than UTC, a
malformed time", and no year bound. The bound is written down only in
`docs/internals/agent/adapter.md`, in the paragraph that begins "So does a nearest candidate whose
year is outside 0000 to 9999".

`standards/13_standards_documentation_and_observability.md:24`: "A change to behaviour,
configuration, events, persisted data, CLI output or a supported platform updates the design
(`design/`) and any user documentation in the same pull request. A code comment or review note is
never the only record of a contract."

## Why it is accepted

- Unwitnessed, and P3: no failing test, reproduction or mutation witness.
- That standards sentence carries no `MUST`.
- No real clock reaches year 10000; #402's description says so. The internal notes add that only a
  clock read in 9999 or later can meet the bound.
- The behaviour is the repair of `PR380-RESET-PARSER-NON-RFC3339-YEAR`, and its boundary tests pin
  it: `a_reset_in_the_year_10000_gives_none` (`src/agent/adapter.rs:1316`) and
  `a_reset_on_the_last_day_of_9999_is_four_digits` (`:1328`).
- #402 is a P3 batch with no further code or design run (ruling r42, step 7).

## What a change that takes this up should do

One sentence in DESIGN §13's "Rate limits and resets, as built" paragraph, beside the sentence on
the nearest occurrence: a nearest occurrence outside the years 0000 to 9999 gives no reset. No code
change.
