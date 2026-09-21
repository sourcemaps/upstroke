---
id: PR286-PROSE-SCANS-CANNOT-SEE-WHAT-A-READER-SEES
severity: P2
disposition: deferred
category: security-trust
pr: 286
reviewed_sha: db94a8ba600dc4547e44e62a5a19ca2f17868aa4
location: scripts/pr-review-parse.py:250 and scripts/pr-review-parse.py:486
provenance: pre_existing
first_bad:
guard: a change that decides how this program reads comment prose at all
---

> Found by #286's review at `553cfddf2cee8d36115414372e0d0763be3d9cad`, as the P1
> `PR286-PROSE-SCANS-READ-THE-WRITTEN-SPELLING`. Every measurement below was re-executed at
> `db94a8ba600dc4547e44e62a5a19ca2f17868aa4`, which is this file's `reviewed_sha`.

## Failure sequence

`PROSE_VERDICT` and `stray_summary` compare **the characters a review comment is stored as**. A
reader of that comment sees **what GitHub's renderer resolves them to**, and in the comment's inline
prose those are two documents. A spelling that renders as the token and is stored otherwise is past
both scans.

    a comment whose prose says `VERDICT&#58; CHANGES_REQUIRED` over a clean `PASS` object
      parser exit 0, verdict PASS  --  written literally: parser exit 1, no result

    a comment whose prose says `The blocker is &#80;1 and it is not in the object.`
      parser exit 0, stray none, the audit READY  --  written literally: stray P1, the audit MANUAL

This is the divergence the P1 `PR286-PROSE-SCANS-READ-THE-WRITTEN-SPELLING` described. That file is
closed by the pull request that files this one, which took the third of the three directions it
offered: **narrow what the two scans are for**, state the limit where each scan is defined, and stop
treating a miss as a bypass. What remains open is the divergence itself, recorded here so that the
change which decides how this program reads comment prose has something to close.

**The limit is confined to inline prose.** Inside a code span or a code block a renderer resolves
nothing, so a reader sees the characters these scans read and the two agree. Measured with
`markdown-it-py` 3.0.0 on 2026-09-21: ```` ```text ```` fencing `The blocker is &#80;1 here.` renders
`The blocker is &amp;#80;1 here.`, and the code span `` `&#80;1` `` renders `<code>&amp;#80;1</code>`.

## Measured

Every row below was executed at `db94a8ba600dc4547e44e62a5a19ca2f17868aa4`, 2026-09-21, against the
tree's own `scripts/pr-review-parse.py`, with `python3` 3.12.3. Each comment carries the prose shown
and then one clean `PASS` verdict object; `exit`, `verdict` and `stray` are that program's `review`
subcommand. The rendering is `markdown-it-py` 3.0.0's, taken as the text a reader sees with the tags
stripped.

| prose the comment stores | what a reader sees | exit | verdict | stray |
|---|---|:-:|:-:|:-:|
| `VERDICT&#58; CHANGES_REQUIRED` | `VERDICT: CHANGES_REQUIRED` | 0 | PASS | none |
| `VERDICT\: CHANGES_REQUIRED` | `VERDICT: CHANGES_REQUIRED` | 0 | PASS | none |
| `V*ERDICT:* CHANGES_REQUIRED` | `VERDICT: CHANGES_REQUIRED` | 0 | PASS | none |
| `VER<span>DICT:</span> CHANGES_REQUIRED` | `VERDICT: CHANGES_REQUIRED` | 0 | PASS | none |
| `VERDICT: CHANGES_REQUIRED` | `VERDICT: CHANGES_REQUIRED` | **1** | — | — |
| `The blocker is &#80;1 ...` | `The blocker is P1 ...` | 0 | PASS | none |
| `The blocker is P**1** ...` | `The blocker is P1 ...` | 0 | PASS | none |
| `The blocker is P1 ...` | `The blocker is P1 ...` | 0 | PASS | **P1** |

Four spellings of `VERDICT:` and two of `P1`, each rendering to the token and each invisible to the
scan; the literal spelling of each is caught. `.github/scripts/test-pr-ready-audit.sh` holds three of
these rows as fixtures under this id, so the divergence is executable at every head rather than only
recorded here: resolving character references in `stray_summary` moves the `[both encoded]` row
(measured, exit 1, `stray` `-` to `P1`), and resolving them before the `PROSE_VERDICT` search moves
`[both encoded]` and `[severity literal]` (measured, exit 1, both to a refused parse).

## Why this is P2 and not P1

**The bar is the owner's ruling of 2026-09-11: a P1 blocks a merge only if it can happen in normal
use, or someone without push access can trigger it.** Neither limb is met. The push-access limb is
measured; the normal-use limb is a judgement about the shapes this finding is about, and is written
below as a judgement rather than as an enumeration:

- **Someone without push access cannot reach either scan.** `scripts/pr-ready-audit.sh` parses
  exactly one comment per pull request: the id comes from `latest_review_id`, whose listing runs
  `review_comment_filter`, whose predicate keeps only comments whose `user.login` lowercases to the
  trusted reviewer's; the body is then fetched by that id. `ledger_result`, the only other thing the
  parser reads, uses neither scan. Enumerated by
  `grep -n 'comments\|gh api\|gh pr\|pr-review-parse' scripts/pr-ready-audit.sh`, which returns 35
  lines at this head: the three that carry a comment or body into the parser are `:613`, the
  filtered listing; `:956`, the body of the id that listing returned, into the file the `review`
  subcommand reads at `:965`; and `:1128`, the pull request's own body, into the `ledger`
  subcommand at `:1132`. `grep -n 'run_review_parser ' scripts/pr-ready-audit.sh` returns those two
  call sites and the function's own comment, and nothing else. Asserted end to end by
  `MUT-REVIEWER-ANY-AUTHOR-READ`: the comment in the first table, written by `a-contributor`, is
  `NOT-READY`, `blockers=no-review`, zero `gh pr merge` calls; written by the trusted reviewer it is
  read. Removing the login predicate from that filter turns the first of those into READY with one
  merge call.
- **The spellings this finding is about are not ones normal use produces.** Each either splits a
  token with markup (`P**1**`, `V*ERDICT:*`, `VER<span>DICT:</span>`) or spells one of its
  characters as an escape or a reference (`&#80;1`, `VERDICT&#58;`, `VERDICT\:`). A reviewer who
  emphasises a whole token is still caught: `**P1**` gives `stray` `P1` and `**VERDICT:**` is found
  by `PROSE_VERDICT`, both measured. **This is a judgement about the shapes above and not an
  enumeration of what has been posted**, which is not derivable from the tree and which I did not
  enumerate; `grep -c 'P\*\*1\|V\*ERDICT\|VER<span>' .github/scripts/test-pr-ready-audit.sh`
  returns 0 at this head. **And it is not a claim about every spelling either scan misses**: one
  that normal use does produce, `_P1_`, is missed for a different reason and is filed separately as
  `PR286-UNDERSCORE-IS-A-WORD-CHARACTER-TO-THE-STRAY-SCAN`. What bounds that one, and this one, to
  P2 is the sentence below rather than the reachability.

**What is left is a divergence between what a reader sees and what the program reads**, and an
account that can write the prose can write the verdict object instead and say anything in it. **A
miss by either scan never changes a verdict**: the workflow form's verdict is its object's, the
prose form's is its `VERDICT:` line inside the review, and what these two scans decide is whether
the parse refuses and whether a person is asked to look. So the ceiling on a miss is a
self-contradictory review by the trusted reviewer being acted on as its object says, instead of
being put in front of a person. The
same reasoning `PR283-COMPARISON-CANNOT-SEE-WHAT-A-READER-CAN` records for the gate's own state
walk: a row of this shape is a P1 **because of a claim of enforcement**, and P2 once the bound is
stated where a reader of the code will see it. It is stated at:

- `scripts/pr-review-parse.py:287` -- `PROSE_VERDICT`: what the scan is for, that the verdict object
  is the authority, that it is not a trust boundary, and the four spellings;
- `scripts/pr-review-parse.py:523` -- `stray_summary`: a net whose catch goes to a person, the same
  limit, and that a miss is the limit rather than a defect to be patched spelling by spelling;
- `scripts/pr-ready-audit.sh:181` -- both scans as nets rather than gates, and the property the
  audit does rest on;
- `.github/scripts/test-pr-ready-audit.sh:7169` -- the section that executes both halves.

**If those sentences are ever dropped, or turned back into a claim that either scan stops a comment
the reader would read differently, this row should be raised.**

## What the change that takes this up should do

**Decide how this program reads comment prose, rather than adding a spelling.** Adding one is what
this defect's own history is made of: each round taught the recogniser one more shape and the next
round found the next. The directions, and what each costs, are unchanged from the P1 this replaces:

- **Render the comment.** Complete, and it puts a Markdown implementation and a choice of renderer
  between the review and the merge decision. This parser is stdlib-only and CI installs nothing for
  it, and "what a reader sees" is not one document: measured for #286 round 4, `markdown-it-py`
  3.0.0 and cmark-gfm 0.29.0.gfm.6 disagree about a fence tagged `json` followed by U+0085, U+000B
  or U+00A0.
- **Refuse prose the program cannot read plainly.** Fail-closed, and on the evidence in the P1 it
  refuses every comment in `.github/scripts/test-pr-ready-audit.sh` and every comment the workflow
  has posted. That is a product decision and it has not been made.
- **Leave the scans as nets and close the divergence elsewhere.** The reading the pull request that
  filed this one took, as far as it goes: the scans are documented, not widened. What it does not do
  is give a reader of a comment and this program one document to agree about, and that is what is
  still owed.

Whichever is taken, the fixtures under this id in `.github/scripts/test-pr-ready-audit.sh` move with
it, and `findings/PROCESS.md`'s rule applies: the row says which direction was chosen.
