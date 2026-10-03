---
id: PR331-A-RESUME-THAT-STOPS-BEFORE-ITS-REPORT-NAMES-NO-KEPT-PIN
severity: P3
disposition: deferred
category: correctness
pr: 331
reviewed_sha: 9b2262f4f2b515cc1181ed67df7f19756612b8a7
location: reviews/2026-10-02-pr11-follow-up-d-record.md:596
provenance: introduced_by_feature
first_bad: cb1beacfd1b7f05519f3460d8bda9c5037f79d8e, follow-up D's implementation of the kept pin's warning, as its design has stated the limitation since 37e4d8c43258e739da6a37b6a44bd51b8494ee18 (R-D5)
guard: a change to how the legacy engine delivers a failing command's warnings (rendering the partial report a failed drain already writes, or carrying the warnings on the error), or the owner's explicit acceptance; until then the residual stays stated as R-D5 in `reviews/2026-10-02-pr11-follow-up-d-record.md` and filed here, and nothing accepts it
---

## Failure sequence

Follow-up D's residual R-D5 (its record §1.4, narrowed in §2.3 and §2.11), with the delivery limits design review
round 4's restore lens named as FUD-D4-WARNDELIVERY (P3, reasoned;
`/home/ubuntu/orch-pr11/reviews/review-331-d4-restore-9b2262f4.review.md`, finding 4). D's implementation corrected the
promise that overstated it (`design/15`'s paragraph now says when the resume reaches its lookup); it did not change the
delivery, which is this file.

1. A registry refusal keeps attempt 1's candidate at its `-kept` pin, and names the pin in the refusal itself.
2. A later resume never names it when it stops before its report:
   - **Refused before the lookup.** While a registration stays torn, the resume's reclaim of the snapshot's intent
     (`reclaim_gate_workspaces`, `src/engine/resume.rs:426`) refuses as a registry refusal, before the kept-pin lookup
     (`:562`). So every such resume names nothing; the refusal's own text names the store and the deadline.
   - **Failed after the lookup.** A resume that reaches the lookup and then fails — a worker that cannot spawn, a second
     refusal — returns its error without its warnings, as every legacy warning does: `drain_and_report` writes the
     partial report, warnings included, to the run's directory (`src/engine/coordinator.rs:342-350`), and the command
     does not render it on error.
3. The operator who reads only the command's output does not learn of the pin from those invocations.

**What already bounds it (executed by the implementation's tests).** The pin is never removed by a resume, and the next
resume that reports names every pin the log records, not only those of attempts still in flight
(`a_kept_pin_is_named_after_a_resume_that_failed`, `every_kept_pin_is_named_after_a_second_refusal_on_the_next_resume`).
`git for-each-ref 'refs/upstroke/prepared/*/*-kept'` lists every kept pin in the repository.

## Severity: P3

Nothing is lost: the pin survives, the refusal that wrote it named it, and the next resume that reports names it again.
What is missing is a notice on the invocations between. The restore lens graded the overstated promise P3, and this is
that promise's remaining gap.

## What the change that takes this up should do

Deliver a failing legacy command's warnings: render the partial report the failed drain already writes, or carry the
warnings on the error (`UpstrokeError::WithWarnings` exists for that shape). A resume that refuses at its reclaim could
also look the pins up before it refuses, which is a change to the frozen legacy resume's order and needs its own owner
decision, as D's did. Nothing here is proposed.
