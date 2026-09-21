---
id: PR286-PROSE-SCANS-CANNOT-SEE-WHAT-A-READER-SEES
severity: P2
disposition: deferred
category: security-trust
pr: 286
reviewed_sha: 196ecd1a6f0932126251cc7aa1dae5de2e0d260a
location: scripts/pr-review-parse.py:949

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
> comment -- beside the body, and the two scans read the text that HTML shows.
>
> **ROUND SEVEN KEEPS THAT DESIGN AND REPAIRS THREE DEFECTS IN IT, EACH EXECUTED AS A MERGE AT
> `7767c71c`**: a `<pre>` GitHub renders raw HTML inside was not read at all, the exemption asked
> whether the comment writes a LINE rather than where, and the body and the rendering were two
> fetches that could return two versions. Every measurement below was executed against round
> seven's head unless it names another.

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
  writes that piece's line AT OR AFTER THE ONE IT LAST MATCHED;
- `<pre>` IS read, and the code blocks THE COMMENT WRITES are taken out of the reading afterwards
  (`quoted_code`). A fenced block's content is shown verbatim, so the comment's own characters
  already are what a reader sees there and the first of the three readings holds it; RAW `<pre>`
  HTML is not verbatim -- GitHub passes the tags inside it through -- and that is read here because
  nothing else reads it. What the narrowing is for is the workflow form's own verdict object, which
  would otherwise be reported as a severity written outside the findings;
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

**3. AN OCCURRENCE THE COMMENT WRITES AND NO READER SEES CAN BE SPENT ON ONE A READER DOES, AND
THIS ONE COSTS A MERGE.** Round six's item 3 asked whether the comment writes a correction's LINE
ANYWHERE; round seven matches the occurrences IN ORDER instead, so an earlier quotation is spent on
the occurrence a reader sees in it and a later correction has only its own spelling to match
against. What is left is narrower and is not closed. A `VERDICT:` the comment writes INSIDE A LINK
REFERENCE DEFINITION'S TITLE is shown to nobody, and a correction a reader does see whose line is
the same characters can be matched to it:

    <!-- upstroke-frontier-review pr=999 head=... -->
    **VERDICT: PASS**

    Nothing blocks.

    VERDICT&#58; CHANGES_REQUIRED

    [ref]: https://example.invalid/x "VERDICT: CHANGES_REQUIRED"

    VERDICT: PASS

A reader sees `VERDICT: CHANGES_REQUIRED` between the two PASS lines. **Executed: exit 0, verdict
PASS, `stray=null`, READY and ONE `gh pr merge` call -- AT `7767c71c` AND at round seven's head,
which give the same answer.** It is not round seven's regression and not round seven's repair.

**ORDER CANNOT TELL THE TWO APART AND NEITHER COULD A COUNT**: one written occurrence disappears as
the definition is rendered and one appears as the reference is resolved, so the two cancel exactly
-- which is round three's counting failure, in the one shape identity does not reach either. What
would tell them apart is knowing which written characters the renderer dropped, and that is a
Markdown parse, which is the thing this design exists not to do. Two rows are fixtured, one with
the definition after the correction and one with it before, under
`MUT-PROSE-VERDICT-EXEMPTION-BY-TEXT [OPEN: ...]`.

**4. The merge path depends on the rendering arriving.** ONE call per review where there were two
before round six and three in it: `application/vnd.github.full+json` returns `created_at`, `body`
and `body_html` together, and `pr-review-parse.py comment` splits that one answer into the two
documents the parse reads. The audit BLOCKS rather than merging when it does not arrive:
`review-fetch-failed` when the call fails, `review-document-unreadable` when the answer is not one
comment, `review-rendering-missing` when it carries no rendering or a blank one, and a parser
refusal when the comment has text and its rendering shows none. That is the product decision, made
in #310 round 6 under `ORCH-P1-RESTART.md` §4 and recorded in that pull request's body: it does NOT
fall back to the stored text, because that fall back is the defect this whole family is about.

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

## What round 7 closed

**Three classes, each executed as a merge at `7767c71c` and each MANUAL with none at round seven's
head.** Every row is fixtured.

| the comment writes | what GitHub shows | at `7767c71c` | at round 7's head |
|---|---|---|---|
| `<pre><strong>VERDICT</strong>: CHANGES_REQUIRED</pre>` over a clean `PASS` | `VERDICT: CHANGES_REQUIRED` | stray none, **READY, one merge call** | stray `VERDICT:`, MANUAL, none |
| `<pre>The blocker is &#80;1 here.</pre>` over a clean `PASS` | `The blocker is P1 here.` | stray none, **READY, one merge call** | stray `P1`, MANUAL, none |
| `` `VERDICT: CHANGES_REQUIRED` `` quoted, then `VERDICT\: CHANGES_REQUIRED` appended | the quotation, then the correction | stray none, **READY, one merge call** | stray `VERDICT:`, MANUAL, none |

**The `<pre>` premise was false and it was round six's own.** That round skipped `<pre>` entirely,
on the ground that a code block's content is shown VERBATIM, so the comment's characters are
already what a reader sees there. GitHub renders RAW HTML inside `<pre>` -- the recorded rendering
of the first row is `<pre class="notranslate"><strong>VERDICT</strong>: CHANGES_REQUIRED</pre>` --
so the tags are operative and the character references resolve. `quoted_code` replaces the premise
with a test: a code block whose shown text the comment WRITES is taken out of this reading, because
the first of the three readings holds it; one the comment does not write is read here.

**And a fourth, in the audit rather than the parser.** `scripts/pr-ready-audit.sh` fetched the
comment's `body` and its `body_html` in two calls, so a reviewer editing between them gave the
audit VERSION A'S CHARACTERS AND VERSION B'S RENDERING -- a pair that passes where each version
alone blocks. Executed against the whole audit: A (`_P1_` in prose over a clean `PASS`) is MANUAL,
B (that finding moved into the object, verdict CHANGES_REQUIRED) is NOT-READY, and A's body beside
B's rendering was PASS, READY and ONE `gh pr merge` call, with neither the comment id nor the
reviewed sha changing. One fetch under `application/vnd.github.full+json` is one version.

**Measured.** Over the 685 comments this repository held on 2026-09-21T20:49:34Z, **0 of 685 change
any field** between `7767c71c` and round seven's head. Over the 126 documents the gate ships a
recorded rendering for, **5 change and every one of them GAINS a token**; the five are the three
rows above plus the second `<pre>` row in the frontier form and a second quotation row.

## Why this is P2 and not P1

**The bar is the owner's ruling of 2026-09-11: a P1 blocks a merge only if it can happen in normal
use, or someone without push access can trigger it.**

**The push-access limb is measured and not met.** `scripts/pr-ready-audit.sh` parses exactly one
comment per pull request: the id comes from `latest_review_id`, whose listing runs
`review_comment_filter`, whose predicate keeps only comments whose `user.login` lowercases to the
trusted reviewer's; GitHub's whole answer for that id is then fetched once and split into the two
documents. `ledger_result`, the only other thing the parser reads, uses neither scan.
`grep -n 'run_review_parser ' scripts/pr-ready-audit.sh` returns THREE call sites and the
function's own comment, and nothing else: `comment` and `review`, which are the two halves of
reading that one comment, and `ledger`, which reads the pull request body.

**Asserted end to end by `MUT-REVIEWER-ANY-AUTHOR-READ`**: the witness comment written by
`a-contributor` is `NOT-READY`, `blockers=no-review`, zero `gh pr merge` calls, and the clean `PASS`
object attributed to the same outsider is READY, enqueued, one merge call once the login predicate
is deleted. Both directions executed.

**The normal-use limb, and item 3 is the one that has to answer it.** 1 costs a `manual:` line if
GitHub emits a tag this classification does not hold, 2 costs nothing, and 4 costs a block rather
than a merge. **3 COSTS A MERGE**, and what keeps it here rather than at P1 is what the document
has to be:

- the review is written in the frontier form (the workflow form refuses any literal `VERDICT:`
  outside its object, so this shape has no reading there at all);
- the reviewer writes their correction in a spelling only a reader sees -- `VERDICT&#58;`,
  `VERDICT\:`, `**VERDICT**:` -- which is ordinary;
- AND the same comment carries a LINK REFERENCE DEFINITION, or a link title, whose text is
  character for character the correction's rendered line, `VERDICT: CHANGES_REQUIRED` and nothing
  else, and which no reader is shown.

The third is not something a reviewer writes. It is not "a quotation of the protocol", which is
what round seven closed: a quotation is SHOWN, so it is spent on the occurrence a reader sees in
it. It is an occurrence written where a reader sees nothing, whose line is exactly the correction's
line. **There is no instance of it in the 685 comments this repository holds** -- measured by
parsing every one at both heads, 0 of 685 differ -- and none in the gate's 126 documents but the
two fixtures that exist to record it.

**A weaker sentence that is true, in place of round six's.** Round six wrote "none of the four is a
reading that shows a reader a severity and reports none". That is still true OF A SEVERITY -- the
severity scan has no exemption at all, so a token in any of the three readings is reported, and the
only silence left there is a tag in neither set, which is itself reported. It is NOT true of a
`VERDICT:` line, which does have an exemption, and item 3 is where it fails.

## What the change that takes this up should do

**Item 3, and it needs something this design does not have.** Round seven's reading agrees with
round six's on all 685 comments and closed three classes that each cost a merge; the shape that is
left is item 3, it costs a merge, and closing it means telling a written occurrence a reader sees
from one a reader does not -- which is knowing what the renderer dropped, which is a Markdown
parse. Two things would do it without one, and both are outside this file:

- **ask GitHub for the correspondence rather than for the result.** Nothing in the REST API offers
  it today; if a rendering ever carries the source offsets of the runs it copied, the exemption
  becomes exact and item 3 closes;
- **or take the exemption away and pay for it.** Every rendered `VERDICT:` would be reported unless
  the comment writes it literally at that point, which makes the frontier form's own two verdict
  lines the only exempt ones and costs a `manual:` line on any review that quotes the protocol.
  Measured: 250 of the 685 comments carry that form's marker. That is a product decision and not a
  repair, so it belongs to the owner.

The other two places to look if a new shape turns up:

- **the tag classification**, which is the only place a wrong answer is silent-ish (it reports,
  rather than reading wrong, so "silent" is the wrong word -- but a person still has to look);
- **`quoted_code`**, which decides which code blocks this reading keeps. It compares the block's
  shown text against what the comment writes, with line endings folded; a block the comment writes
  that this comparison misses would be READ, which is the reporting direction, and one the comment
  does not write that it matches anyway would be DROPPED, which is not.

**AND WHATEVER IS TAKEN, MEASURE IT AGAINST GITHUB'S OWN RENDERING OF THE COMMENTS THIS REPOSITORY
HOLDS.** Five rounds of this finding were closed by a review and not by a test, and the two defects
round five found itself were invisible to `markdown-it-py` because GitHub is not `markdown-it-py`.
The corpus is one `gh api` call:

    gh api "repos/sourcemaps/upstroke/issues/comments?per_page=100" \
      -H 'Accept: application/vnd.github.full+json' --paginate \
      --jq '.[] | {id, created_at, login: .user.login, body, body_html}'

Whichever is taken, the fixtures under this id in `.github/scripts/test-pr-ready-audit.sh` move
with it, and `findings/PROCESS.md`'s rule applies: the row says which direction was chosen.
