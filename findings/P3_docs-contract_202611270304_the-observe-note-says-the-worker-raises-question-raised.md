---
id: PR435-OBSERVE-NOTE-QUESTION-ORIGIN
severity: P3
disposition: deferred
category: docs-contract
pr: 435
reviewed_sha: 5ca9531f56e6eb0483bb43f6fb01e3672a0dab44
location: docs/internals/engine/topology/observe.md:90
provenance: introduced_by_feature
first_bad: introduced on this branch (PR #435) with the view `upstroke decide` reads; the note is not in origin/master's tree (552c0395)
guard: the next change to `Observed::raised_questions` or its note, or to how `upstroke decide` labels who raised a question (`RaisedBy::of`)
---

## Failure sequence

From PR #435's round-two regression review, group 8, filed under ruling R79 at P3.

    the observe note describes the view upstroke decide reads, and who raised each question
    -> it names `question_raised` as the worker's
    -> the coordinator writes `question_raised`, to hold an escalation onto the frontier rung for
       a person's approval of the spend
    -> upstroke decide labels a question opened by `question_raised` as the coordinator's
    -> so the note gives the event the opposite origin from the code that reads it

The sentence, at 5ca9531f `docs/internals/engine/topology/observe.md:88`-`:92` (line 90 is the
one the review cites), and on the same lines at the repaired head:

```text
`open_question_candidates` finds them. `open_questions` cannot give that order,
because the fold keys its open questions by id, and only the opening event says
who raised the question: the worker (`question_raised`, or an attempt parked on
a worker's failure), a reviewer (an attempt parked on a reviewer's failure), or
integration (a spawn, a merge repair or a verification that could not run). It
```

The coordinator writes it, at `src/engine/topology/run.rs:1749` (`ask_to_approve_spend`):

```rust
        self.emit(
            TopologyEventBody::QuestionRaised {
                data: QuestionRaised4 { question },
            },
```

`RaisedBy::of` labels everything but an attempt parked on a worker's `NeedsHuman` failure the
coordinator's, at 5ca9531f `src/decide.rs:572` and after the repair `src/decide.rs:755`, and
`decide::tests::a_question_is_labelled_by_whether_the_worker_or_the_coordinator_raised_it`
asserts the case `a spend approval` (a `QuestionRaised` event) is `RaisedBy::Coordinator`.
`docs/internals/decide.md` (`enum RaisedBy {`) says the same as the code.

Nothing executes the note; the mismatch is between two sentences and the code, read here.

## What the change that takes this up should do

- Say who raised each opening event as `RaisedBy::of` reads it: the worker only for an attempt
  parked on its own `NeedsHuman` failure; `question_raised` the coordinator's, for a spend
  approval.
- Keep the note's other attributions (a reviewer, integration) consistent with the label the
  prompt gives them, or say that the view's three-way reading and decide's two-way label differ.
