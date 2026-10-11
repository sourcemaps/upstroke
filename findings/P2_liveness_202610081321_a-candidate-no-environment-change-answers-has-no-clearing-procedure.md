---
id: PR329-R14-A-CANDIDATE-NO-ENVIRONMENT-CHANGE-ANSWERS-HAS-NO-CLEARING-PROCEDURE
severity: P2
disposition: deferred
category: liveness
pr: 329
reviewed_sha: 492325c4bcf37c1c0bcd51ab223c9729b1aa60bc
location: src/engine/topology/run.rs:779
provenance: introduced_by_feature
first_bad: PR #329's design round 8 (its record §7.3, R14), implemented at 58c7c203; prior ID PR329-A-GENUINE-CHECKOUT-FAILURE-AFTER-THE-TAKEOVER-REFUSES, whose accepted liveness cost this is the unsupported part of
guard: before any change that activates the topology (PR12) and before G6: a supported operator procedure, implemented and tested, that clears, skips or gives up a candidate whose verification snapshot no environment change lets its host check out (as the question master's park raised let an operator do), or the owner's explicit acceptance of that stuck run as part of R14's cost; graded by B's review, under the owner's O9(a) of 2026-10-08 ("gaps are filed and handled under existing review/repair rules")
---

## Failure sequence

Filed at PR #329's B11 round, which states R14's operator view and clearing procedure for the owner's O9(a) (the
follow-up B record's §9.28): the procedure it can state does not reach this case, and no change in that round's scope
can close it.

    a schema-4 run's candidate is proposed for integration, and its proposal holds a path the verification
    snapshot's checkout can never make on this host, whatever the configuration: for example a name longer than the
    filesystem's limit (executed: `tree_with_a_name_too_long`'s 300-byte name, "File name too long", on Linux)
    -> the snapshot's `git worktree add` fails after Git took the destination over, Git's junk removal takes the
       destination, and the access refuses at once, `UpstrokeError::RegistryRefused`, naming the destination
       (`…/snapshots/s<N>-integration`), the commit and Git's whole standard error
    -> `run::verified` passes the refusal on (`src/engine/topology/run.rs:779` at 492325c4), so no unavailable
       terminal is appended, no deferral is spent and no question is raised; the command ends, with nothing durable
       recording why
    -> each resume settles the verification `merge_verification_interrupted` with recovery's fixed detail, verifies
       the same proposal again under a new sequence, meets the same failure and refuses again
    -> the run stops at every resume; nothing this tree supports clears, skips or gives up that candidate: no
       command addresses a candidate, and `upstroke answer --decline` needs a question none of these raises

Where master returned the add's failure as Git state, `verified`'s Git arm (`:769`) settled the sequence unavailable,
the candidate deferred and then parked with a question at `max_defers`, and an operator could answer it
(`PR329-A-GENUINE-CHECKOUT-FAILURE-AFTER-THE-TAKEOVER-REFUSES`, its failure sequence). B's refusal at once (§7.3, R14)
removes that lever for this case. The owner accepted R14's liveness cost under O9(a) on 2026-10-08, with the duty that
B's record state a supported clearing procedure, "No unproved clearing procedure is approved", and gaps filed.

**What is executed and what is reasoned.**
- **Executed:** the refusal's text for the 300-byte name, at an integration gate snapshot (`s7-integration`), an
  integration review snapshot (`s7-review1`) and an attempt's gate snapshot (`k3-g1-a2-gates`), through
  `add_snapshot`, in a private copy of 492325c4 with one probe test added
  (`~/orch-pr11/logs/pr11_fub_impl16/o9/probe.log`, `probe.patch`); the existing tests that the add refuses after one
  attempt and leaves nothing (`an_add_whose_checkout_cannot_be_made_refuses_after_one_attempt_and_leaves_nothing`),
  and that a verification whose snapshot checkout fails after the takeover ends the command resumably and re-verifies
  once the environment is repaired
  (`a_verification_whose_snapshot_checkout_fails_after_the_takeover_ends_resumably_and_its_resume_reverifies`).
- **Reasoned:** that the same proposal fails at every resume when no environment change answers its content; that no
  command reaches such a candidate. No run was driven through repeated resumes over such content.
- **Not reachable today:** no supported entry point starts or resumes the schema-4 topology at 492325c4 (the record's
  §9.28 exposure determination), so no operator can meet this before an activation.

## What the change that takes this up should do

Give the operator a supported way to clear such a candidate before any activation: for example, raise the question
master's park raised once a verification's snapshot has refused this way on consecutive resumes, so that an answer can
give the task up; or a command that gives up a named candidate. Test it with the probe's content, red on this head.
Or the owner accepts the stuck run by name as part of R14's cost. Environmental causes are outside it: a failing
filter, a missing object, a destination the filesystem will not write, and a path length that a configuration change
such as Windows' long paths answers, each cleared by repairing the environment and resuming (the record's §9.28).
