---
id: PR286-PROSE-SCANS-READ-THE-WRITTEN-SPELLING
severity: P1
disposition: deferred
category: security-trust
pr: 286
reviewed_sha: 553cfddf2cee8d36115414372e0d0763be3d9cad
location: scripts/pr-review-parse.py:773 and scripts/pr-review-parse.py:451
provenance: pre_existing
first_bad:
guard: a change that decides how this program reads comment PROSE at all
---

## Failure sequence

PR #286 closes two guards that compared the characters a review comment is stored as while the only
consumer of what they guard compared what those characters decode to. Two more scans in the same
file have the same shape and are **not** closed by it, because their consumer is GitHub's Markdown
renderer rather than a decoder.

**1. The `VERDICT:` line outside the verdict block.** `the_verdict_block` refuses a comment that
carries both a verdict object and a verdict line, because such a comment says two things and the
program would be choosing between them. `PROSE_VERDICT` searches the raw characters.

    a comment carrying `VERDICT&#58; CHANGES_REQUIRED` and a clean `PASS` object
      at 553cfddf:  parser exit 0, verdict PASS   audit exit 0, READY, one mocked merge call
      the same line written literally:  parser exit 1, zero merge calls

**2. The stray severity scan.** `stray_summary` exists to catch a blocking severity written where
the findings are not, and sends such a review to a person. It reads two spellings -- what the
comment writes and what `json.loads` decodes -- and not what a renderer resolves.

    a comment whose prose says `The blocker is &#80;1 and it is not in the object.`
      at 553cfddf:  stray = none, the audit READY
      the same severity written literally:  stray = P1, the audit MANUAL

Both render as the token a reader sees. Measured with `markdown-it-py` 3.0.0 on this box,
2026-09-14.

## Why #286 does not take this up

#286's rule is that a guard asks its question of what its own consumer reads, and it is a **closure**
for the two guards it repairs because each of those consumers performs a complete, small
transformation: a renderer gives a fence the first word of its info string once backslash escapes
and character references are resolved, **and nothing else** -- the info string is not parsed as
Markdown -- and `json.loads` resolves a JSON string's escapes. Both are implementable in full. The
first is implemented as `markdown-it-py` 3.0.0's own function rather than as a reading of the
specification, which does not define a fence's language: three readings of the specification were
each wrong somewhere that renderer is not (#286 rounds 2 to 4).

The consumer here is the whole inline renderer, and resolving character references would close
**one spelling of at least four**. All four were measured at 553cfddf, each rendering to the token
and each invisible to the raw scan:

| spelling | renders as |
|---|---|
| `VERDICT&#58;` / `&#80;1` | character reference |
| `VERDICT\:` | backslash escape |
| `V*ERDICT:*` / `P**1**` | emphasis |
| `VER<span>DICT:</span>` | inline raw HTML |

Adding the first row alone is the enumeration this file's own history identifies as the defect --
"each time the recogniser was made to understand one more shape, and each time the next round found
the next shape". It would leave the other three and read, in the gate, as a closure.

## What the change that takes this up should do

Decide what this program may assume about comment prose at all, rather than adding a spelling.
Three directions, none of them free:

- **Render the comment.** Complete, and it puts a Markdown implementation between the review and the
  merge decision. The parser is stdlib-only today and CI installs nothing for it. It also has to
  choose a renderer, because "what a reader sees" is not one document: measured for #286 round 4,
  `markdown-it-py` 3.0.0 gives a fence tagged `json` and then a literal U+0085, U+000B or U+00A0 the
  language `json`, and cmark-gfm 0.29.0.gfm.6 does not.
- **Refuse prose the program cannot read plainly.** Fail-closed, and it has to separate the rows
  that must stay green: every real workflow-form comment carries a preamble, and a rule that
  refuses one refuses them all. The json form reads its `reviewed_sha` from the object, not from the
  `Reviewed head:` line, so "nothing but whitespace before the block" is *arithmetically* available
  -- but it would refuse every comment in `.github/scripts/test-pr-ready-audit.sh` and, on the
  evidence available here, every comment the workflow has posted. That is a product decision.
- **Narrow what these two scans are for.** `stray_summary` is a net, not a gate -- the verdict object
  is the authority and a severity outside it only sends the review to a person. `PROSE_VERDICT` is a
  self-contradiction detector, and an attacker who can write the verdict object need not write a
  contradicting line at all. If that is the intended reading, say it where the scans are defined and
  stop treating a miss as a bypass. It is still a divergence between what a reader sees and what the
  program reads, and the ledger should say which of the three was chosen.
