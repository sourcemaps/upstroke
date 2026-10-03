---
id: PR331-KEPT-PINS-ACCUMULATE-UNTIL-THE-OPERATOR-REMOVES-THEM
severity: P3
disposition: deferred
category: performance
pr: 331
reviewed_sha: 9b2262f4f2b515cc1181ed67df7f19756612b8a7
location: reviews/2026-10-02-pr11-follow-up-d-record.md:598
provenance: introduced_by_feature
first_bad: cb1beacfd1b7f05519f3460d8bda9c5037f79d8e, follow-up D's implementation of the kept pin, as its design has stated the limitation since 37e4d8c43258e739da6a37b6a44bd51b8494ee18 (R-D7)
guard: the owner's ruling on kept-pin retention, or a change that retires a kept pin once its output is committed or recovered; until then the operator removes each with `git update-ref -d <pin>`, and the residual stays stated as R-D7 in `reviews/2026-10-02-pr11-follow-up-d-record.md` and filed here; nothing accepts it
---

## Failure sequence

Follow-up D's residual R-D7 (its record §1.4 and §1.8, and §3.2 for a cause that persists).

1. A legacy attempt's gate or review snapshot is refused by the worktree registry after the worker's output was
   captured. The coordinator pins the captured candidate at the attempt's prepared pin followed by `-kept`
   (`src/engine/coordinator.rs:555`), one pin per refused attempt.
2. No resume removes a kept pin (`src/engine/resume.rs`; `a_registry_refusal_after_capture_keeps_and_pins_the_captured_candidate_across_both_resumes`).
3. Over a cause that persists — a registration that stays torn, or a genuine failure after Git took the destination
   over, which D's veto refuses at once and keeps (record §3.2) — every resume pays one more worker attempt, as master's
   did, and keeps one more pin (`every_kept_pin_is_named_after_a_second_refusal_on_the_next_resume` keeps two).
4. Each pin keeps its candidate's objects reachable, and every later resume of the run names it and pays three Git
   processes per recorded attempt to look it up (record §2.3). They accumulate until the operator removes them.

## Severity: P3

By design the operator, not the engine, decides what to do with a kept candidate, and the warning says how to remove
it (`git update-ref -d <pin>`, after which no resume names it: `a_removed_kept_pin_is_named_no_more`). The cost is
growth of refs and retained objects in the user's repository and of the resume's lookups, one pin per refused attempt;
nothing is lost.

## What the change that takes this up should do

Decide a retention rule: for example, retire a kept pin once a later attempt of the same task commits, or once its
output has been recovered, with the warning saying so. A rule that removes a pin the operator has not acted on would
remove the only durable copy of paid output, which is what D exists to keep, so it is the owner's call. Nothing here is
proposed.
