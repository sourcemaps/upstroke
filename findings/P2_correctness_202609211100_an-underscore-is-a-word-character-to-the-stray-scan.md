---
id: PR286-UNDERSCORE-IS-A-WORD-CHARACTER-TO-THE-STRAY-SCAN
severity: P2
disposition: deferred
category: correctness
pr: none
reviewed_sha: db94a8ba600dc4547e44e62a5a19ca2f17868aa4
location: scripts/pr-review-parse.py:175
provenance: pre_existing
first_bad:
guard: a change that decides what ends a stray token, and what a review comment may cite without being sent to a person
---

> Found while taking up `PR286-PROSE-SCANS-READ-THE-WRITTEN-SPELLING`, not by a review. It is a
> different mechanism from `PR286-PROSE-SCANS-CANNOT-SEE-WHAT-A-READER-SEES`, which is why it is a
> separate file: nothing here turns on what a renderer resolves. The scan misses a token that is
> plainly there **in the characters it reads**.

## Failure sequence

`STRAY_TOKEN` is `\b(?:P[0-3]|MUST)\b` under `re.ASCII`, and **`_` is a word character**, so `\b`
finds no boundary between an underscore and a letter. A severity or a MUST wrapped in underscores
is therefore no token at all to the scan whose whole job is to notice one written outside the
findings.

    a comment whose prose says `The remaining issue is _P1_ and it is out of scope.`
      over a clean PASS object:  parser exit 0, verdict PASS, stray none  ->  the audit is READY

    the same sentence with `P1` written plainly:  stray P1  ->  the audit is MANUAL

`_MUST_` behaves the same way. Measured at `db94a8ba600dc4547e44e62a5a19ca2f17868aa4`, 2026-09-21,
with `python3` 3.12.3 running the tree's own `scripts/pr-review-parse.py review`.

**Underscore emphasis is ordinary Markdown and an ordinary thing to write.** `markdown-it-py` 3.0.0
renders `_P1_` as `<em>P1</em>` and `_MUST_` as `<em>MUST</em>`, so a reader of the comment sees the
token; and unlike every spelling in
`PR286-PROSE-SCANS-CANNOT-SEE-WHAT-A-READER-SEES`, a reader of the **raw** comment sees it too. The
token is not split and no character of it is escaped. Nothing about this needs a renderer.

## Why the obvious repair is not obviously right

Widening the boundary to `(?<![0-9A-Za-z])(?:P[0-3]|MUST)(?![0-9A-Za-z])` catches both, and it also
catches **a finding filename cited in prose**:

| text | `\b(?:P[0-3]\|MUST)\b` | the wider boundary |
|---|---|---|
| `the remaining issue is _P1_ and it is out of scope` | none | `P1` |
| `a _MUST_ deviation in touched code` | none | `MUST` |
| `see findings/P1_liveness_202609031117_intermittent-kill-settle-and-residue-failures.md` | none | `P1` |
| `the branch fix-P1/security-trust_two-prose-scans` | `P1` | `P1` |
| `plain P1 here` | `P1` | `P1` |

A review comment citing `findings/P1_...` by path is an ordinary comment, and under the wider
boundary every one of them is `manual:` and goes to a person. The last two rows are the other half
of the tension: a **branch** name already trips the scan today, because `/` ends a word and `_` does
not, so the scan already treats two ordinary citations differently for a reason nobody chose.

`re.ASCII` was chosen deliberately, and the file says why: *"The safe direction for a stray-token
scan is to find MORE of them, because each one is a blocker."* That sentence argues for the wider
boundary; the filename row argues against it. **Which of the two is right is a product decision
about what a review comment may cite without being sent to a person, and it has not been made.**

## What the change that takes this up should do

Decide what ends a stray token, and say so where `STRAY_TOKEN` is defined. The two candidates
measured above are the boundary as it is and the wider one; a third is a boundary that ends a word
at `_` but not where the underscore is part of a filename-shaped run, which is a rule about
citations rather than about tokens and should be written as one if it is chosen. Whichever is taken,
a fixture per row of the table above belongs in `.github/scripts/test-pr-ready-audit.sh`, because
the table is the whole of the decision.

Do not take it up by adding `_P1_` to a list of spellings: this is the boundary rule, not a
spelling, and the row above that must stay green is the filename one.
