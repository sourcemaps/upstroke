---
id: PR386-PROFILE-REFUSAL-TRAILING-PERIOD
severity: P3
disposition: deferred
category: docs-contract
pr: 386
reviewed_sha: e68b4225bbbc4acc25bf20913ee1c3ff9a44ded4
location: src/engine/topology/preflight.rs:256
provenance: introduced_by_feature
first_bad: bc1f63202ee01c769019dd07eaf016bc66a021b7
guard: the next P3 batch in engine/topology
---

## Failure sequence

- `profile_refused` (`src/engine/topology/preflight.rs:255-256`) ends its message with `again.`,
  and `UpstrokeError::Refused` displays its message whole (`src/error.rs:152-153`). So every profile
  refusal's `Display` text ends in a period. That is against the rule that `Display` text has no
  trailing period (`standards/07_standards_errors_and_panics.md:8`), a line with no `MUST`.
- Master's `refused` (`src/engine/topology/preflight.rs:273`) ends in a period too, as these
  refusals have since `bc1f6320`.
- Ruling r26 dictated the sentence, not an exemption from the rule.

## What the change that takes this up should do

- Drop the final period of `profile_refused` and of `refused`.
- Move the two tests that pin it: `PROFILE_REFUSAL_CLOSING`
  (`src/engine/topology/preflight/tests.rs:540-543`) and the `ends_with` assertion in
  `src/engine/tests.rs:27564-27567`.
