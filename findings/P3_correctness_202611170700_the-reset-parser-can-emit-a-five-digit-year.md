---
id: PR380-RESET-PARSER-NON-RFC3339-YEAR
severity: P3
disposition: deferred
category: correctness
pr: 380
reviewed_sha: 2be3e9586e6995f5e1b3cc4e4ced3b4f7b9a0480
location: src/agent/adapter.rs:299
provenance: introduced_by_feature
first_bad:
guard: the change that makes the parser refuse a year outside RFC 3339's four digits, or any change to `rfc3339_utc`'s range
---

# The reset parser can emit a five-digit year

## Failure sequence

Round nine's frontier review found this by reading the code. It was not run, and no test
reproduces it.

`reset_at_from_limit_message` in `src/agent/adapter.rs` picks the candidate instant nearest to
`now_unix` and, on its last line (line 299 at the reviewed commit), formats that instant with
`crate::util::rfc3339_utc`. `rfc3339_utc` (`src/util.rs`) writes the year with `{year:04}`, which
pads to at least four digits but does not stop at four. Nothing between them checks the year.

    now_unix = 253402300800, which is 10000-01-01T00:00:00Z
    -> the five-hour message, `· resets 5:30am (UTC)`, has no date, so the candidates are
       05:30 UTC yesterday, today and tomorrow
    -> the nearest is 10000-01-01 05:30 UTC
    -> `rfc3339_utc` formats it as `10000-01-01T05:30:00Z`
    -> the function returns `Some("10000-01-01T05:30:00Z")`

RFC 3339 allows only a four-digit year. The function's documentation promises an RFC 3339 UTC
timestamp, `YYYY-MM-DDTHH:MM:SSZ` (`docs/internals/agent/adapter.md`, under the function's
heading), and the coordinator records the value as `reset_at` on the `pool_exhausted` event. A
strict consumer can therefore reject a reset time the log recorded as valid.

The function takes `now_unix` without restriction, but this sequence needs a clock past
9999-12-31.

## What the change that takes this up should do

Return `None` when the chosen instant's year is outside 0000–9999, and do not hand
`rfc3339_utc` a time whose year it cannot write in four digits. Add a test at that boundary: the
message above at `now_unix = 253402300800` answers `None`, and an instant that falls on
9999-12-31 still gives a four-digit timestamp.
