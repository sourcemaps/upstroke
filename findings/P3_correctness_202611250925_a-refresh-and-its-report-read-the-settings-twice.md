---
id: PR421-REFRESH-REPORT-DOUBLE-CAPTURE-RACE
severity: P3
disposition: accepted-risk
category: correctness
pr: 421
reviewed_sha: 517763346fdb3d20f77fba18742c2eefce7aed32
location: src/tracker.rs:112
provenance: introduced_by_feature
first_bad: 18d9206e, #421's repair of PR421-REFRESH-REPORT-SECTIONS-STILL-GOVERN
guard: the next change to tracker::refresh, or to how upstroke capacity --refresh calls it
---

## Failure sequence

From #421 round four's frontier review (comment 7154793002), reasoned, not run:

    upstroke capacity --refresh captures a configuration the report accepts
    -> while refresh asks the providers, another process atomically replaces upstroke.toml
       with a [capacity] value the report refuses
    -> refresh stores the providers' results and returns; the CLI prints them
    -> capacity::report takes its own capture and refuses
    -> the command fails after the effects the comment in refresh says a refused file prevents

`src/main.rs` calls `tracker::refresh` (:294) and then `capacity::report` (:306), and each reads
the files through `config::load_for_report` on its own. The comment in `refresh`
(`src/tracker.rs:112` to `:115`) says "a file the report would refuse stops the command here,
before any provider is asked or anything is stored". That holds for a file that is unchanged
between the two captures; the test
`a_refresh_refuses_what_its_report_would_before_any_provider_is_asked` (`src/tracker.rs`)
covers only that case.

## Why it is accepted

- Unwitnessed, and P3: no failing test, reproduction or mutation witness.
- It needs the settings file replaced during the providers' calls, between two captures in one
  command. What is stored is the providers' own usage readings, which the next refresh replaces.
- The report it precedes is read-only, and the refusal it then prints is the one a run would give.
- The owner chose this disposition on 2026-11-25 (option (a) for #421).

## What the change that takes this up should do

Not proposed here; the guard names the change that takes it up.
