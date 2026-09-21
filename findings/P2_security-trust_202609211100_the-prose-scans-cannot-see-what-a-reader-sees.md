---
id: PR286-PROSE-SCANS-CANNOT-SEE-WHAT-A-READER-SEES
severity: P2
disposition: deferred
category: security-trust
pr: 286
reviewed_sha: a5bcc998a2c75abb60ecd905da8ff3765aba2b9c
location: scripts/pr-review-parse.py:615
provenance: pre_existing
first_bad:
guard: a change that decides how this program reads a comment's inline constructs, not only its characters
---

> Found by #286's review at `553cfddf2cee8d36115414372e0d0763be3d9cad`, as the P1
> `PR286-PROSE-SCANS-READ-THE-WRITTEN-SPELLING`. Filed in #310 round 1 as the whole divergence, with
> a severity argument that #310's round-1 review disproved by execution. **REWRITTEN AT
> `a5bcc998a2c75abb60ecd905da8ff3765aba2b9c` to what is actually left**, which is a different and
> much smaller set: every measurement below was executed against the head of #310 round 2.

## What this is now

`PROSE_VERDICT` and `stray_summary` read three spellings of a review comment: what it stores, what
`json.loads` decodes, and -- since #310 round 2 -- **what a reader of the inline prose sees**,
through `reader_spelling`. That third reading resolves a backslash escape, a character reference and
an emphasis delimiter run, by `markdown-it-py` 3.0.0's own `escape`, `entity` and `scanDelims`.

**It is not a renderer**, and this finding is the three places it is not, measured. Two of them can
still hide a token from a reader's eye; two of them find a token a reader does not see; not pairing
delimiters does both.

## Failure sequence -- the class that is still not read

**An inline construct standing BETWEEN two characters of the token.** `reader_spelling` resolves
escapes, references and delimiter runs; it does not read raw HTML and it does not read link syntax,
so either of those splitting a word leaves the token in the rendering and none of it in any reading.

    a comment whose prose says `VER<span>DICT:</span> CHANGES_REQUIRED` over a clean `PASS` object
      parser exit 0, verdict PASS, stray none  ->  the audit is READY, one merge call
      a reader sees `VERDICT: CHANGES_REQUIRED`

    the same with `VER[DICT:](https://example.invalid/x)`
      parser exit 0, verdict PASS, stray none  ->  the audit is READY
      a reader sees `VERDICT: CHANGES_REQUIRED`, the first three letters linked

    `The blocker is P<span>1</span> here.` and `The blocker is P[1](https://example.invalid/x) here.`
      stray none, where a reader sees `P1`

This is the same failure the P1 described and it now needs the token to be **cut in half by a tag or
a link**. What was closed in #310 round 2 is everything that does not cut it: `**VERDICT**:`,
`VERDICT&#58;`, `VERDICT\:`, `V*ERDICT:*`, `_P1_`, `_MUST_`, `&#80;1` and `P**1**` are each read now
and each was invisible at `db94a8ba`.

## Failure sequence -- the two classes that are read WIDER than a renderer reads them

Both cost a `manual:` blocker and a person's attention. Neither can cost a merge.

**Delimiters are not paired, and `stray_summary` answers that by scanning the text with every run
DROPPED and again with every run KEPT.** Those are the two answers a renderer gives, so a sentence
whose runs it answers all the same way is covered in both directions:

    `The class is P*1 in the table.`   stray `P1`, where a reader sees `P*1`  (over-read)

**A sentence whose runs a renderer answers DIFFERENTLY is not**, and that is an under-read:

    `Deferred: __&#80;1**and__ the rest of it.`   stray none
      a reader sees `Deferred: P1**and the rest of it.` -- the `__` pair removed, the `**` left
      written, and `P1` standing between a space and an asterisk

Differential-tested against `markdown-it-py` 3.0.0 over 400,000 random strings on 2026-09-21:
**7 of them** are of that shape. **198** were missed when only the dropped reading was scanned, and
every one of those 198 was the other half of the same gap -- a run this drops and a renderer keeps,
taking a word boundary with it (`U**&#80;0` is `U**P0` to a reader and read `UP0` here). Scanning
both answers closed 191 of the 198 and is why the kept reading exists.

**A code span and a code block are read as prose.** A renderer resolves nothing inside either, and
all three readings resolve everything everywhere.

    `` `&#80;1` ``            stray `P1`, where a reader sees `&#80;1`
    `` `_P1_` ``              stray `P1`, where a reader sees `_P1_`
    a ```` ```text ```` fence holding `_P1_`   stray `P1`, where a reader sees `_P1_`

**The second of those is not new and predates this file.** `decoded_spelling` has always resolved
JSON escapes over the whole comment, so `` `P\u0031` `` and the same text in a `text` fence have
reported `stray=P1` since long before either reading was added -- measured at
`a5bcc998a2c75abb60ecd905da8ff3765aba2b9c` and at `db94a8ba600dc4547e44e62a5a19ca2f17868aa4` alike.
**#310 round 1 claimed the opposite** -- that reader and scan agree inside a code span or a code
block -- in this file, in that pull request's body and at `PROSE_VERDICT`. That claim was wrong when
it was written; #310's round-1 review found it by execution and it is corrected in all three places.

## Measured

Executed at `a5bcc998a2c75abb60ecd905da8ff3765aba2b9c` (the base) and at #310 round 2's head, with
`python3` 3.12.3 running each tree's own `scripts/pr-review-parse.py review`. Each comment carries
the prose shown and then one clean `PASS` verdict object. The rendering is `markdown-it-py` 3.0.0's,
taken as the text a reader sees with the tags stripped.

| prose the comment stores | what a reader sees | at `a5bcc998` | at this head |
|---|---|:-:|:-:|
| `VER<span>DICT:</span> CHANGES_REQUIRED` | `VERDICT: CHANGES_REQUIRED` | none | none |
| `VER[DICT:](https://example.invalid/x) ...` | `VERDICT: CHANGES_REQUIRED` | none | none |
| `The blocker is P<span>1</span> here.` | `The blocker is P1 here.` | none | none |
| `The blocker is P[1](https://example.invalid/x) here.` | `The blocker is P1 here.` | none | none |
| `The class is P*1 in the table.` | `The class is P*1 in the table.` | none | **P1** |
| `Deferred: __&#80;1**and__ the rest of it.` | `Deferred: P1**and the rest of it.` | none | none |
| `` The blocker is `&#80;1` here. `` | ``The blocker is `&#80;1` here.`` | none | **P1** |
| `` The blocker is `_P1_` here. `` | ``The blocker is `_P1_` here.`` | none | **P1** |
| `` The blocker is `P\u0031` here. `` | ``The blocker is `P\u0031` here.`` | **P1** | **P1** |
| a `text` fence holding `_P1_` | `_P1_` | none | **P1** |
| a `text` fence holding `P\u0031` | `P\u0031` | **P1** | **P1** |

Seven of these eleven rows are pinned as fixtures in `.github/scripts/test-pr-ready-audit.sh` under
this id: rows 1, 2, 5, 6, 7, 8 and 10. The four that are not are the two that split a SEVERITY
rather than the verdict line (rows 3 and 4), which are the same two constructs as rows 1 and 2 and
add no class, and the two `P\u0031` rows (9 and 11), whose behaviour is `decoded_spelling`'s and is
unchanged by this pull request. The two under-read rows are driven through the WHOLE AUDIT as READY with one
merge call each, because a quiet `stray` field is not that claim. The next change starts from a
measurement rather than a guess.

## Why this is P2 and not P1

**The bar is the owner's ruling of 2026-09-11: a P1 blocks a merge only if it can happen in normal
use, or someone without push access can trigger it.**

**The push-access limb is measured and not met.** `scripts/pr-ready-audit.sh` parses exactly one
comment per pull request: the id comes from `latest_review_id`, whose listing runs
`review_comment_filter`, whose predicate keeps only comments whose `user.login` lowercases to the
trusted reviewer's; the body is then fetched by that id. `ledger_result`, the only other thing the
parser reads, uses neither scan. Enumerated by
`grep -c 'comments\|gh api\|gh pr\|pr-review-parse' scripts/pr-ready-audit.sh`, which returns 35 at
this head; the three that carry a comment or a body are `:623`, the filtered listing, `:966`, the
body of the id that listing returned, into the file the `review` subcommand reads at `:975`, and
`:1138`, the pull request's own body, into the `ledger` subcommand at `:1142`.
`grep -n 'run_review_parser ' scripts/pr-ready-audit.sh` returns those two call sites and the
function's own comment, and nothing else. Asserted end to end by `MUT-REVIEWER-ANY-AUTHOR-READ`:
the witness comment written by `a-contributor` is `NOT-READY`, `blockers=no-review`, zero
`gh pr merge` calls; removing the login predicate from that filter turns it into READY with one.

**The normal-use limb is a judgement, and the last judgement of this kind made here was wrong, so
this one says exactly what it rests on.** #310 round 1 argued that every spelling in this family
"splits a token with markup" and so was not normal use. That was false of `**VERDICT**:`, which
WRAPS the token rather than splitting it, and wrapping a word in bold is what a reviewer does when
correcting a generated review. The round-1 review executed that and it was READY with one merge
call. **That is why those classes are closed in code rather than argued about here.**

What is left is narrower, and the distinction is the one round 1 got wrong, drawn the right way
round: **the construct must fall BETWEEN two characters of the token, and be one a writer has no
reason to put there.** `VER<span>DICT:</span>` and `VER[DICT:](url)` are not shapes that come out of
writing a sentence -- there is no correction, no emphasis and no citation that produces them. **This
is a judgement about those two constructs and not an enumeration of what has been posted**, which is
not derivable from the tree. What IS derivable, and was measured: over the 677 comments this
repository holds, fetched from `repos/sourcemaps/upstroke/issues/comments`, not one changes outcome
between the two heads; and `git grep -l 'VER<span>\|VER\[DICT'` names three tracked files at this
head and no others -- `.github/scripts/test-pr-ready-audit.sh`, `scripts/pr-review-parse.py` and
this file -- every hit in them a fixture or a sentence about one, and none a review that was posted.

**The mixed-pairing under-read is bounded by measurement rather than by judgement**: 7 in 400,000
random strings drawn from an alphabet deliberately loaded with `*`, `**`, `_`, `__` and `&#80;`, and
0 in the 677 comments this repository holds. It needs a severity written encoded or split AND two
delimiter runs in one sentence that a renderer answers differently. **It is a P2 on the same footing
as the rest of this row and it should be raised if a shape of it turns out to be ordinary writing.**

**And it is not a claim that nothing else is missed.** The class searched for was *an inline
construct that deletes characters between the token's letters*, and the two members found by reading
CommonMark's inline rules against `reader_spelling` are raw HTML and link syntax. An autolink
(`<https://example.invalid/P1>`) keeps its text and is read; an image (`P![1](...)`) puts the `1` in
an `alt` attribute, so a reader does not see `P1` either. **A different lens would be a different
class**, and if one turns up that ordinary writing produces, this row is a P1 again.

**What bounds the two WIDER readings to P2 is different and simpler**: each sends a review to a
person, and neither can produce a merge. The cost they carry is attention, measured across the whole
corpus of comments this repository has: 677 comments fetched from
`repos/sourcemaps/upstroke/issues/comments`, every one of them by the trusted reviewer, **none of
which changes outcome between the two heads**; and the 142 comment fixtures
`.github/scripts/test-pr-ready-audit.sh` builds, of which exactly the two carrying this finding's own
witnesses change.

## What the change that takes this up should do

**Decide whether this program reads a comment's inline STRUCTURE, rather than adding a construct.**
The three readings each answer to one complete transformation; structure is a different thing, and
reading it means knowing where a code span begins, which delimiters pair, and where a link's text
ends -- which is a CommonMark inline parse, and this program is stdlib-only with CI installing
nothing for it.

- **Render the comment.** Complete, and it puts a Markdown implementation and a choice of renderer
  between the review and the merge decision. "What a reader sees" is not one document: measured for
  #286 round 4, `markdown-it-py` 3.0.0 and cmark-gfm 0.29.0.gfm.6 disagree about a fence tagged
  `json` followed by U+0085, U+000B or U+00A0.
- **Refuse prose the program cannot read plainly.** Fail-closed, and on the evidence in the P1 it
  refuses every comment in `.github/scripts/test-pr-ready-audit.sh` and every comment the workflow
  has posted. That is a product decision and it has not been made.
- **Narrow the two wider readings without widening the narrow one**, which is the cheapest of the
  three and closes none of the merge-affecting class: a code-span and code-block scan would stop
  the over-reads, and is itself a piece of structure.

Whichever is taken, the fixtures under this id in `.github/scripts/test-pr-ready-audit.sh` move with
it, and `findings/PROCESS.md`'s rule applies: the row says which direction was chosen.
