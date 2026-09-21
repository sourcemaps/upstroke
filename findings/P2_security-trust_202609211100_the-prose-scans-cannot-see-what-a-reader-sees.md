---
id: PR286-PROSE-SCANS-CANNOT-SEE-WHAT-A-READER-SEES
severity: P2
disposition: deferred
category: security-trust
pr: 286
reviewed_sha: d599216669fd8e9e079f857a68ae3199a2222fd3
location: scripts/pr-review-parse.py:830
provenance: pre_existing
first_bad:
guard: a change that decides how this program reads a comment's inline constructs, not only its characters
---

> Found by #286's review at `553cfddf2cee8d36115414372e0d0763be3d9cad`, as the P1
> `PR286-PROSE-SCANS-READ-THE-WRITTEN-SPELLING`. Filed in #310 round 1 as the whole divergence, with
> a severity argument that #310's round-1 review disproved by execution; rewritten at `a5bcc998` to
> what round 2 left. **REWRITTEN AGAIN AT `d599216669fd8e9e079f857a68ae3199a2222fd3`**, whose three
> reviews found that a CODE SPAN and a LINK wrapping a label whole were still a merge, and that the
> frontier form never got the contradiction check at all. Both are closed in #310 round 3. Every
> measurement below was executed against that round's head.

## What this is now

`PROSE_VERDICT` and `stray_summary` read **six** spellings of a review comment: what it stores, what
`json.loads` decodes, and what a reader of the inline prose sees under each combination of the two
questions one reading cannot settle -- whether a renderer PAIRED a delimiter run or left it written,
and whether the comment's code spans and links are CONSUMED or written. `reader_spelling` resolves a
backslash escape, a character reference and an emphasis delimiter run by `markdown-it-py` 3.0.0's
own `escape`, `entity` and `scanDelims`; `markup_regions` reads that renderer's `backtick` rule and
CommonMark's four link forms, with a reference link a link only where a definition defines its
label.

**It is not a renderer**, and this finding is what it is not, measured. **Two classes can still hide
a token from a reader's eye; five are read wider than a renderer reads them.**

## Failure sequence -- the two classes that can still cost a merge

**Inline raw HTML.** No reading parses an HTML tag, so a tag standing between two of the token's
characters leaves the token in the rendering and none of it in any reading.

    a comment whose prose says `VER<span>DICT:</span> CHANGES_REQUIRED` over a clean `PASS` object
      parser exit 0, verdict PASS, stray none  ->  the audit is READY, one merge call
      a reader sees `VERDICT: CHANGES_REQUIRED`

**A sentence whose two delimiter runs a renderer answers DIFFERENTLY.** Delimiters are not paired;
`stray_summary` answers that by scanning with every run dropped AND again with every run kept, which
is every answer a renderer gives when it answers them all the same way. It does not cover a
renderer that pairs one run and leaves another written in the same sentence.

    `Deferred: __&#80;1**and__ the rest of it.`   stray none
      a reader sees `Deferred: P1**and the rest of it.` -- the `__` pair removed, the `**` left
      written, and `P1` standing between a space and an asterisk

**What was closed in #310 round 3** is everything that WRAPS the label rather than cutting it with a
tag: `` `VERDICT`: ``, `[VERDICT](url):`, `VER[DICT:](url)`, `` P`1` `` and `P[1](url)` are each read
now, at the parser and through the whole audit, and each was READY with one `gh pr merge` call at
`d599216`. **And the frontier form is now asked the same question**: `VERDICT: PASS` with
`**VERDICT**: CHANGES_REQUIRED` appended was READY with one call there too.

## Failure sequence -- the five classes that are read WIDER than a renderer reads them

Each costs a `manual:` blocker and a person's attention. None can cost a merge.

    `The class is P*1 in the table.`     stray `P1`, where a reader sees `P*1`
    `` The blocker is `&#80;1` here. ``  stray `P1`, where a reader sees `` `&#80;1` ``
    `` The blocker is `_P1_` here. ``    stray `P1`, where a reader sees `` `_P1_` ``
    a ```` ```text ```` fence holding `_P1_`   stray `P1`, where a reader sees `_P1_`
    `[VERDICT]: https://example.invalid/review-policy`   stray `VERDICT:`, where a reader sees
      NOTHING: that line is a link reference definition and a renderer shows none of it
    `Note [a [b](u) P](v)1 here.`       stray `P1`, where a reader sees `Note [a b P](v)1 here.`:
      links do not nest, so a renderer leaves the OUTER pair written and this does not

The first four are the readings that leave a code span's delimiters written resolving inside it,
and the two `markup_regions` adds are new with round 3. **The code-span row is not wholly new and
predates this file**: `decoded_spelling` has always resolved JSON escapes over the whole comment, so
`` `P1` `` and the same text in a `text` fence have reported `stray=P1` since long before any
of these readings were added -- measured at `a5bcc998` and at `db94a8ba` alike. **#310 round 1
claimed the opposite** -- that reader and scan agree inside a code span or a code block -- and that
claim was wrong when it was written.

## Measured

Executed at `d599216669fd8e9e079f857a68ae3199a2222fd3` (round 2's head) and at round 3's head, with
`python3` 3.12.3 running each tree's own `scripts/pr-review-parse.py review`. Each comment carries
the prose shown and then one clean `PASS` verdict object. The rendering is `markdown-it-py` 3.0.0's,
taken as the text a reader sees with the tags stripped.

| prose the comment stores | what a reader sees | at `d599216` | at this head |
|---|---|:-:|:-:|
| `VER<span>DICT:</span> CHANGES_REQUIRED` | `VERDICT: CHANGES_REQUIRED` | none | none |
| `Deferred: __&#80;1**and__ the rest of it.` | `Deferred: P1**and the rest of it.` | none | none |
| `` `VERDICT`: CHANGES_REQUIRED `` | `VERDICT: CHANGES_REQUIRED` | none | **VERDICT:** |
| `[VERDICT](https://example.invalid/p): ...` | `VERDICT: CHANGES_REQUIRED` | none | **VERDICT:** |
| `VER[DICT:](https://example.invalid/x) ...` | `VERDICT: CHANGES_REQUIRED` | none | **VERDICT:** |
| `` The blocker is P`1` here. `` | `The blocker is P1 here.` | none | **P1** |
| `The blocker is P[1](https://example.invalid/x) here.` | `The blocker is P1 here.` | none | **P1** |
| `The class is P*1 in the table.` | `The class is P*1 in the table.` | **P1** | **P1** |
| `` The blocker is `&#80;1` here. `` | ``The blocker is `&#80;1` here.`` | **P1** | **P1** |
| `` The blocker is `_P1_` here. `` | ``The blocker is `_P1_` here.`` | **P1** | **P1** |
| a `text` fence holding `_P1_` | `_P1_` | **P1** | **P1** |
| `[VERDICT]: https://example.invalid/review-policy` | (nothing) | none | **VERDICT:** |
| `Note [a [b](u) P](v)1 here.` | `Note [a b P](v)1 here.` | none | **P1** |

**Every row of this table has a fixture of the same shape** in
`.github/scripts/test-pr-ready-audit.sh`, thirteen of them: rows 3 to 7 under
`MUT-STRAY-READS-THE-WRITTEN-SPELLING`, which differ from the prose above only in carrying the
sentence a reviewer would have written around them, and rows 1, 2 and 8 to 13 under this id, which
are the prose above. **The two rows
that are still under-read are driven through the WHOLE AUDIT as READY with one merge call each**,
because a quiet `stray` field is not that claim. The frontier form's own witness -- the appended
`**VERDICT**: CHANGES_REQUIRED`, and the same correction as a code span, a link and a character
reference -- is `MUT-PROSE-VERDICT-UNCHECKED`, with an ordinary two-verdict-line prose review as
its control.

**Differentially tested against `markdown-it-py` 3.0.0 over 400,000 random strings** built from
fragments a token can hide in, on 2026-09-21. An under-read is a token or a `VERDICT:` line the
renderer shows and no reading holds: **19 at this head against 45 at `d599216`**, and every one of
the 19 holds a `*` or `_` run. With every such run taken out of the generator and 400,000 strings
drawn again: **0 here, 40 there.** That is the measurement behind "the residue is the delimiter
pairing and nothing else".

## Why this is P2 and not P1

**The bar is the owner's ruling of 2026-09-11: a P1 blocks a merge only if it can happen in normal
use, or someone without push access can trigger it.**

**The push-access limb is measured and not met.** `scripts/pr-ready-audit.sh` parses exactly one
comment per pull request: the id comes from `latest_review_id`, whose listing runs
`review_comment_filter`, whose predicate keeps only comments whose `user.login` lowercases to the
trusted reviewer's; the body is then fetched by that id. `ledger_result`, the only other thing the
parser reads, uses neither scan. Enumerated by
`grep -c 'comments\|gh api\|gh pr\|pr-review-parse' scripts/pr-ready-audit.sh`, which returns 35 at
this head; the three that carry a comment or a body are `:631`, the filtered listing, `:974`, the
body of the id that listing returned, into the file the `review` subcommand reads at `:983`, and
`:1146`, the pull request's own body, into the `ledger` subcommand at `:1150`.
`grep -n 'run_review_parser ' scripts/pr-ready-audit.sh` returns those two call sites (`:983`,
`:1150`) and the function's own comment (`:252`), and nothing else.

**Asserted end to end by `MUT-REVIEWER-ANY-AUTHOR-READ`, and WHICH ROW SAYS SO CHANGED WHEN THE
SCANS LEARNED TO READ.** The witness comment written by `a-contributor` -- a `VERDICT:` line and a
`P1`, both as character references -- is `NOT-READY`, `blockers=no-review`, zero `gh pr merge`
calls. **The earlier revision of this finding said that deleting the login predicate turns that row
into READY with one call. That was true at `a5bcc998` and #310 round 2's own repair falsified it**:
executed on 2026-09-21 by deleting exactly that `select` line, the row is `MANUAL` with **zero**
calls and `blockers=manual:P1/VERDICT:-outside-the-verdict-object`, because the two scans now see
what it hides. The mutation is still caught -- by that row's `no-review` and `NOT-READY`
assertions, and by `MUT-REVIEWER-JQ-INJECTION`, `MUT-REVIEWER-CASE-MISMATCH` and
`MUT-REVIEWER-NULL-AUTHOR` -- but the row that shows a MERGE had to become a comment the scans are
silent about. That row is now in the gate: the clean `PASS` object attributed to `a-contributor`,
`no-review` with no call at this head, and **READY, enqueued, one merge call** with the predicate
deleted. Both directions executed.

**The normal-use limb is a judgement, and the two judgements of this kind made here before were
wrong, so this one says exactly what it rests on.** Round 1 argued that every spelling in this
family "splits a token with markup"; that was false of `**VERDICT**:`, which wraps. Round 2 argued
that a code span is only ever read WIDER than a renderer; that was false in the other direction,
because `` `VERDICT`: `` is read NARROWER -- the label disappears. **Both were closed in code rather
than argued about here, and so was the frontier form's missing check.**

What is left is narrower, and it is drawn the same way round: **the construct must fall BETWEEN two
characters of the token, and be one a writer has no reason to put there.** `VER<span>DICT:</span>`
is not a shape that comes out of writing a sentence -- there is no correction, no emphasis, no
citation and no link that produces it; every construct that a correction or a citation DOES produce
is now read. **This is a judgement about one construct and not an enumeration of what has been
posted**, which is not derivable from the tree. What IS derivable, and was measured: over the 681
comments this repository holds, fetched from `repos/sourcemaps/upstroke/issues/comments`, every one
of them by the trusted reviewer, **not one changes outcome between the two heads**; and
`git grep -l 'VER<span>'` names three tracked files at this head and no others --
`.github/scripts/test-pr-ready-audit.sh`, `scripts/pr-review-parse.py` and this file -- every hit in
them a fixture or a sentence about one, and none a review that was posted.

**The mixed-pairing under-read is bounded by measurement rather than by judgement**: 19 in 400,000
random strings drawn from a generator deliberately loaded with `*`, `**`, `_`, `__`, `&#80;` and
`VERDICT:`, 0 with those runs removed, and 0 in the 681 comments this repository holds. It needs a
severity written encoded or split AND two delimiter runs in one sentence that a renderer answers
differently. **It is a P2 on the same footing as the rest of this row and it should be raised if a
shape of it turns out to be ordinary writing.**

**And it is not a claim that nothing else is missed.** The class searched for was *an inline
construct that deletes characters between the token's letters*, by reading CommonMark's inline rules
against `reader_spelling`; the members found were raw HTML, link syntax and the code span, and the
last two are now read. An autolink (`<https://example.invalid/P1>`) keeps its text and is read; an
image (`P![1](...)`) puts the `1` in an `alt` attribute, so a reader does not see `P1` either. **A
different lens would be a different class**, and if one turns up that ordinary writing produces,
this row is a P1 again.

**What bounds the five WIDER readings to P2 is different and simpler**: each sends a review to a
person, and neither can produce a merge. The cost they carry is attention, measured across the whole
corpus of comments this repository has -- the 681 above, none of which changes outcome -- and across
the gate's own comment fixtures: running the `d599216` gate with this head's parser, **exactly one
comment fixture moves**, `[link splits the token]`, and it moves from unread (READY, one merge call)
to `VERDICT:` (MANUAL, none). No fixture moves from parsing to refusing, and none moves from MANUAL
to READY.

## What the change that takes this up should do

**Decide whether this program reads a comment's RAW HTML, and whether it pairs delimiters.** Those
are the two left, and they are not the same kind of problem: the first is another grammar to
transcribe, the second is the whole of CommonMark's emphasis algorithm, which is where a reading of
characters stops being one.

- **Render the comment.** Complete, and it puts a Markdown implementation and a choice of renderer
  between the review and the merge decision. "What a reader sees" is not one document: measured for
  #286 round 4, `markdown-it-py` 3.0.0 and cmark-gfm 0.29.0.gfm.6 disagree about a fence tagged
  `json` followed by U+0085, U+000B or U+00A0.
- **Refuse prose the program cannot read plainly.** Fail-closed, and on the evidence in the P1 it
  refuses every comment in `.github/scripts/test-pr-ready-audit.sh` and every comment the workflow
  has posted. That is a product decision and it has not been made.
- **Transcribe the `html_inline` rule the way `escape`, `entity`, `scanDelims` and `backtick` were
  transcribed**, which is the cheapest of the three and closes the one class that can still cost a
  merge. It buys nothing against the pairing residue.
- **Narrow the five wider readings**, which closes nothing that costs a merge and is worth doing
  only if the `manual:` lines they cost turn out to be paid. They cost none today: 0 of 681.

Whichever is taken, the fixtures under this id in `.github/scripts/test-pr-ready-audit.sh` move with
it, and `findings/PROCESS.md`'s rule applies: the row says which direction was chosen.
