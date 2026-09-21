---
id: PR286-PROSE-SCANS-CANNOT-SEE-WHAT-A-READER-SEES
severity: P2
disposition: deferred
category: security-trust
pr: 286
reviewed_sha: 196ecd1a6f0932126251cc7aa1dae5de2e0d260a
location: scripts/pr-review-parse.py:636

provenance: pre_existing
first_bad:
guard: a change that decides how this program reads what a reader of a comment sees
---

> Found by #286's review at `553cfddf2cee8d36115414372e0d0763be3d9cad`, as the P1
> `PR286-PROSE-SCANS-READ-THE-WRITTEN-SPELLING`. Filed in #310 round 1 as the whole divergence, with
> a severity argument that #310's round-1 review disproved by execution; rewritten at `a5bcc998`
> (round 2), at `d599216669fd8e9e079f857a68ae3199a2222fd3` (round 3 closed the code span, the link
> and the frontier form's missing check), at `3fe68c37e00beb5a5502f861a2a71be72f619da2` (round 4
> closed a wrapped reference definition, a `)` inside a quoted title, a near label fold and an
> exemption that counted rather than matched) and at
> `196ecd1a6f0932126251cc7aa1dae5de2e0d260a` (round 5 closed a block prefix, a raw HTML tag, both
> answers to a reference pair, GFM strikethrough, `\r\n` line endings and two quadratics).
>
> **REWRITTEN AGAIN IN #310 ROUND 6, AND THE DESIGN IS DIFFERENT.** Rounds 2-5 each answered "what
> does a reader see?" by TRANSCRIBING a renderer's inline rules over the characters the comment
> stores, and each round's repair introduced defects of its own: two in round three, one in round
> four, **three in round five**, which is the rate that made the rule count beside the point. Round
> six deletes the transcription. The audit fetches `body_html` -- GITHUB'S OWN RENDERING of the
> comment -- beside the body, and the two scans read the text that HTML shows. Every measurement
> below was executed against round six's head.

## What this is now

`stray_summary` reads **three** documents: the comment's own characters, what `json.loads` decodes
out of them (`decoded_spelling`, which exists because a severity spelled `"P\u0031"` is `P1` to the
only reader the verdict object has), and **the text GitHub's rendering of the comment shows**.
`unwritten_verdict` reads the third.

The third is `Rendering`, and it is a reduction of HTML rather than a reading of Markdown:

- what stands between the tags is the text; a tag in `INLINE_HTML` is dropped and the runs either
  side of it join, and every other tag ends a line;
- each run GitHub copied out of the comment is kept as its own piece, so `unwritten_verdict` can
  ask whether an occurrence a reader sees stands wholly inside one of them AND whether the comment
  writes that piece's line;
- `<pre>` is not read here at all: a code block's content is shown VERBATIM, so the comment's own
  characters already are what a reader sees there, and reading it here as well would report the
  workflow form's own verdict object as a severity written outside the findings;
- and an ordered list item opening with a severity is a NUMBERED FINDING, which is the frontier
  form's own findings rather than prose outside them.

**It is not a renderer and it does not have to be**: it reads what the renderer produced. What is
left is four things, none of them a reading of Markdown.

## What is left

**1. A tag the reduction cannot place.** Every tag is one a reader sees a boundary at or one a
reader sees none at, and BOTH wrong answers lose a token: `P<em>1</em>` is `P1` to a reader and `P`
and `1` to a reading that breaks at `em`, and `<li>P1</li><li>P2</li>` is two lines to a reader and
`P1P2` -- which carries no severity -- to one that does not break at `li`. Round five lost a
severity the second way, by consuming a `<br>` as markup. So a tag in NEITHER set is reported as
`unreadable-html:<tag>` and the review goes to a person. It costs a `manual:` line and never a
merge, and it costs one only if GitHub's renderer starts emitting a tag this classification does
not hold. **Measured: 0 of the 685 comments this repository held on 2026-09-21T20:49:34Z carry
one.** The census of the 28 tags that do appear is a fixture.

**2. `STRAY_TOKEN` asks for a word boundary, and a renderer can take one away.**
`` The blocker is P`1`[policy]. `` with `[policy]:` defined renders `P1policy`, which carries no
severity token -- and neither does `P1policy` written plainly, so a reviewer writing those words
gets the same answer either way. This is not a divergence between the reading and the reader; it is
the token rule, applied to what the reader sees. Two rows are fixtured.

**3. The exemption asks whether the comment writes that LINE, not where.** `unwritten_verdict`
exempts an occurrence a reader sees when it stands wholly inside one run GitHub copied out of the
comment AND the comment writes that run's line somewhere in the scanned text. A correction whose
rendered line happens to coincide with a line the comment writes elsewhere is therefore exempt. Not
reproduced as a merge: for the workflow form a literal `VERDICT:` line anywhere outside the object
is already a refusal, and for the frontier form `parse_prose_review` reads the LAST verdict run in
the characters, so a correction written plainly enough to coincide is a correction that form has
already read.

**4. The merge path depends on one more API call.** Three calls per review where there were two,
and the audit BLOCKS rather than merging when the third does not arrive: `review-fetch-failed` when
it fails, `review-rendering-missing` when it returns nothing, and a parser refusal when the comment
has text and its rendering shows none. That is the product decision, made in #310 round 6 under
`ORCH-P1-RESTART.md` §4 and recorded in that pull request's body: it does NOT fall back to the
stored text, because that fall back is the defect this whole family is about.

## What round 6 closed

**The one class that could still cost a merge, and five that cost attention.** Each was a row of
`reader_open` at `b0c8b8c916fa6af975b21ca684e4b77ac0f81bf9` and each is fixtured at its new value.

| prose the comment stores | what GitHub shows | at `b0c8b8c9` | at round 6's head |
|---|---|---|---|
| `Deferred: __&#80;1**and__ the rest of it.` | `Deferred: P1**and the rest of it.` | stray none, **READY, one merge call** | stray `P1`, MANUAL, none |
| `` The blocker is `&#80;1` here. `` | `` The blocker is `&#80;1` here. `` | stray `P1` | stray none |
| `` The blocker is `_P1_` here. `` | `` The blocker is `_P1_` here. `` | stray `P1` | stray none |
| `The class is P*1 in the table.` | `The class is P*1 in the table.` | stray `P1` | stray none |
| `[VERDICT]: https://example.invalid/x` | nothing: it is a definition | stray `VERDICT:` | stray none |
| `Note [a [b](u) P](v)1 here.` | `Note [a b P](v)1 here.` | stray `P1` | stray none |
| a ```` ```text ```` fence holding `_P1_` | `_P1_` | stray `P1` | stray none |

**And an under-read `b0c8b8c9` did not know it had.** `The blocker is &#00000080;1 here.` is a
`reader_quiet` row at that head -- `markdown-it-py` 3.0.0 allows seven decimal digits in an inline
character reference and leaves eight written -- and **GitHub resolves eight**: a reader sees `P1`.
It is `stray=P1` here and is fixtured as a closed row rather than a quiet one.

**And round five's own three P1s.** `The unresolved issue is _P1_~_P2_ depending on input size.`
renders `<em>P1</em>~<em>P2</em>` and was `P1P2` to round five's reading, so no severity;
`The blocker is P[1](url)<br>Still reproducible.` was `P1Still`, the same way. Both are `P1`/`P2`
and `P1` here. The third was a fixture that could not fail, and it is round six's gate that fixes
it.

## Why this is P2 and not P1

**The bar is the owner's ruling of 2026-09-11: a P1 blocks a merge only if it can happen in normal
use, or someone without push access can trigger it.**

**The push-access limb is measured and not met.** `scripts/pr-ready-audit.sh` parses exactly one
comment per pull request: the id comes from `latest_review_id`, whose listing runs
`review_comment_filter`, whose predicate keeps only comments whose `user.login` lowercases to the
trusted reviewer's; the body and the rendering are then fetched by that id. `ledger_result`, the
only other thing the parser reads, uses neither scan. `grep -n 'run_review_parser ' scripts/pr-ready-audit.sh`
returns the two call sites and the function's own comment, and nothing else.

**Asserted end to end by `MUT-REVIEWER-ANY-AUTHOR-READ`**: the witness comment written by
`a-contributor` is `NOT-READY`, `blockers=no-review`, zero `gh pr merge` calls, and the clean `PASS`
object attributed to the same outsider is READY, enqueued, one merge call once the login predicate
is deleted. Both directions executed.

**The normal-use limb.** Of the four things left, one costs a `manual:` line if GitHub emits a new
tag (2 and 3 cost nothing and 4 costs a block, not a merge). **None of the four is a reading that
shows a reader a severity and reports none**: the only way that happens now is a tag in neither
set, and that is reported.

## What the change that takes this up should do

**Nothing, until something measures a cost.** Round six's reading agreed with the transcription on
all 685 comments and closed the six open rows; there is no known shape where a reader sees a token
and this reports none. The three places to look if one turns up:

- **the tag classification**, which is the only place a wrong answer is silent-ish (it reports,
  rather than reading wrong, so "silent" is the wrong word -- but a person still has to look);
- **the `<pre>` decision**, which rests on a code block's content being shown verbatim. If that
  ever stops being true -- a renderer that rewrites inside `<pre>` -- the scan would read the
  characters where the reader sees something else;
- **the exemption's line test**, item 3 above.

**AND WHATEVER IS TAKEN, MEASURE IT AGAINST GITHUB'S OWN RENDERING OF THE COMMENTS THIS REPOSITORY
HOLDS.** Five rounds of this finding were closed by a review and not by a test, and the two defects
round five found itself were invisible to `markdown-it-py` because GitHub is not `markdown-it-py`.
The corpus is one `gh api` call:

    gh api "repos/sourcemaps/upstroke/issues/comments?per_page=100" \
      -H 'Accept: application/vnd.github.full+json' --paginate \
      --jq '.[] | {id, created_at, login: .user.login, body, body_html}'

Whichever is taken, the fixtures under this id in `.github/scripts/test-pr-ready-audit.sh` move
with it, and `findings/PROCESS.md`'s rule applies: the row says which direction was chosen.
