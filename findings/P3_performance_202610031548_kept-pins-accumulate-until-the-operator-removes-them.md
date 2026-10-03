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

Follow-up D's residual R-D7 (its record §1.4 and §1.8, and §3.2 for a checkout the snapshot cannot make). *(Narrowed by
#331's repair round 2, item E2 of its early review: item 3 said that a registration that stays torn makes every resume
pay one more attempt and keep one more pin, which the code does not do.)*

1. A legacy attempt's gate or review snapshot is refused by the worktree registry after the worker's output was
   captured. The coordinator pins the captured candidate at the attempt's prepared pin followed by `-kept`
   (`src/engine/coordinator.rs:555`), one pin per refused attempt.
2. No resume removes a kept pin (`src/engine/resume.rs`; `a_registry_refusal_after_capture_keeps_and_pins_the_captured_candidate_across_both_resumes`).
3. **Pins accumulate when a refusal recurs after a successful reclaim.** A resume runs the worker again only past its
   reclaim of the run's snapshots (`reclaim_gate_workspaces`, `src/engine/resume.rs:426`), and each refusal after that
   attempt's capture pays one more worker attempt, as master's did, and keeps one more pin:
   - a checkout the snapshot cannot make after Git took its destination over, whose cleanup succeeds — a genuine
     failure D's veto refuses at once and keeps, so a cause that persists refuses every resume again (record §3.2,
     its v3: three invocations ran the worker three times and kept two pins);
   - a registration torn again during the resumed attempt, after the first tear was repaired
     (`every_kept_pin_is_named_after_a_second_refusal_on_the_next_resume` keeps two).

   **A registration that stays torn does not accumulate pins.** Each resume refuses at its reclaim, before the worker
   runs and before the pin lookup: no attempt, no new pin, the event log unchanged, and exactly one pin surviving two
   such resumes (executed by #331's early review, regular lens,
   `~/orch-pr11/reviews/331-i1-early-witnesses/review331-regular-theo4b_z/static-tear.log`; reproduced at `20e27724` by
   #331's repair round 2, `~/orch-pr11/logs/pr11_fud_impl2/repro/e1e2/test.log`).
4. Each pin keeps its candidate's objects reachable. Every later resume **that reaches its lookup** names it and pays
   three Git processes per recorded attempt to look it up (record §2.3); one that refuses at its reclaim looks nothing up
   and names nothing, and one that fails after its lookup returns without its warnings (R-D5,
   `PR331-A-RESUME-THAT-STOPS-BEFORE-ITS-REPORT-NAMES-NO-KEPT-PIN`). The pins accumulate until the operator removes
   them.

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
