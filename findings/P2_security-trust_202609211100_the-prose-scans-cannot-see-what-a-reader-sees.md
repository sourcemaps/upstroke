---
id: PR286-PROSE-SCANS-CANNOT-SEE-WHAT-A-READER-SEES
severity: P2
disposition: deferred
category: security-trust
pr: 286
reviewed_sha: 196ecd1a6f0932126251cc7aa1dae5de2e0d260a
location: scripts/pr-review-parse.py:700

provenance: pre_existing
first_bad:
guard: a change that decides how this program reads a comment's inline constructs, not only its characters
---

> Found by #286's review at `553cfddf2cee8d36115414372e0d0763be3d9cad`, as the P1
> `PR286-PROSE-SCANS-READ-THE-WRITTEN-SPELLING`. Filed in #310 round 1 as the whole divergence, with
> a severity argument that #310's round-1 review disproved by execution; rewritten at `a5bcc998` to
> what round 2 left, again at `d599216669fd8e9e079f857a68ae3199a2222fd3` (round 3 closed the code
> span, the link, and the frontier form's missing check), and again at
> `3fe68c37e00beb5a5502f861a2a71be72f619da2` (round 4 closed a wrapped reference definition, a `)`
> inside a quoted title, a near label fold, and an exemption that counted rather than matched).
> **REWRITTEN AGAIN AT `196ecd1a6f0932126251cc7aa1dae5de2e0d260a`**, whose two lenses found that
> round 4's own repair had REOPENED a blocked correction inside a blockquote, that the raw-HTML
> remainder is ordinary writing after all, and that the structure pass was still quadratic. Two
> more came out of round 5's own measurement, neither from a lens: GitHub renders STRIKETHROUGH and
> this reading did not, and a comment GitHub's editor stores with `\r\n` endings had no blank lines
> to this reading at all -- which was harmless at `196ecd1a` and which round 5's own repair would
> have made cost a severity. All of them are closed in #310 round 5. Every measurement below was
> executed against that round's head.

## What this is now

`PROSE_VERDICT` and `stray_summary` read **up to fourteen** spellings of a review comment: what it
stores, what `json.loads` decodes, and what a reader of the inline prose sees under each combination
of FOUR questions one reading cannot settle -- whether a renderer PAIRED a delimiter run or left it
written, whether the comment's code spans, links and raw HTML tags are CONSUMED or written, whether
a REFERENCE PAIR is a link or its own brackets, and whether each line's BLOCK PREFIX has been taken
off. The last is six of the fourteen and is made only when some line carries a prefix.

Every rule is a transcription of a renderer's own function rather than a reading of the
specification:

- `reader_spelling` resolves a backslash escape, a character reference and a delimiter run by
  `markdown-it-py` 3.0.0's `escape`, `entity` and `scanDelims`, with `~` on the `*` arm because
  GitHub renders GFM STRIKETHROUGH;
- `markup_regions` reads that renderer's `backtick` rule, its `html_inline` rule and CommonMark's
  four link forms, with each form's EXTENT read by its own `parseLinkDestination`, `parseLinkTitle`
  and label rule -- and with the reference forms read BOTH WAYS, because GitHub and `markdown-it-py`
  disagree about which destinations make a definition and neither answer is safe alone;
- `block_prefix_view` takes a blockquote marker, a list marker and the indentation around them off
  each line, which is what a renderer's block pass does before any inline rule runs;
- and each reading carries the PARTS it was joined out of, so the frontier form's exemption for its
  own verdict lines names the occurrence it exempts rather than counting occurrences.

**It is not a renderer**, and this finding is what it is not, measured. **One class can still hide
a token from a reader's eye; nine are read wider than a renderer reads them.**

## Failure sequence -- the one class that can still cost a merge

**A sentence whose two delimiter runs a renderer answers DIFFERENTLY.** Delimiters are not paired;
`stray_summary` answers that by scanning with every run dropped AND again with every run kept, which
is every answer a renderer gives when it answers them all the same way. It does not cover a
renderer that pairs one run and leaves another written in the same sentence.

    `Deferred: __&#80;1**and__ the rest of it.`   stray none
      a reader sees `Deferred: P1**and the rest of it.` -- the `__` pair removed, the `**` left
      written, and `P1` standing between a space and an asterisk
      parser exit 0, verdict PASS, stray none  ->  the audit is READY, one merge call

It needs a severity written encoded or split AND two delimiter runs in one sentence that a renderer
answers differently. **1 in 100,000 random strings from round 5's own generator**, against 59 at
`196ecd1a`; 0 in the 684 comments this repository holds.

## What round 5 closed, and each was READY with one merge call at `196ecd1a`

**1. A BLOCK PREFIX IS TAKEN OFF BEFORE THE INLINE READING.** A renderer parses block structure
first and hands each block's CONTENT to its inline rules, so inside a blockquote they never see the
`>` that opens each line and inside a list item they never see the indentation. This reading is run
over the comment AS GITHUB STORES IT. **Round 4 introduced the first of these**: reading a link's
title the way that renderer reads it meant meeting the second line's `>` where whitespace or a title
had to stand, so a citation wrapped inside a quote stopped being a link and the correction between
its brackets carried no token.

    > [VERDICT](https://example.invalid/policy
    > "Review"): CHANGES_REQUIRED -- correcting the review below.

    > [VERDICT][policy]: CHANGES_REQUIRED -- correcting the review below.
    >
    > [policy]: https://example.invalid/review-policy

**2. INLINE RAW HTML IS MARKUP AND THE TEXT BETWEEN TWO TAGS IS TEXT.** `<strong>VERDICT</strong>:`
and `<code>VERDICT</code>:` render to the same characters as `**VERDICT**:` and `` `VERDICT`: ``,
which is ordinary writing and not a word split with a tag. The previous revision of this file
deferred the whole raw-HTML remainder on the ground that splitting a word with a tag is not what
writing a sentence produces; that ground was **false of the two spellings above**, and #310's round-4
review executed the rendering equality. `html_tag_extent` is that renderer's `html_inline` rule, and
what it reads is a TAG: a comment, a processing instruction, a declaration and a CDATA section are
consumed WITH their content, because a reader sees none of that either.

**3. WHETHER A REFERENCE PAIR IS A LINK IS NOW SCANNED BOTH WAYS, AND THAT IS THE ROUND'S ONE
DESIGN CHANGE.** Round 4 let a definition's destination wrap onto the next line by asking only that
something non-blank follow the colon, which INVENTED definitions a renderer refuses; a pair naming
an invented label has its brackets consumed, and dropping those brackets JOINS the words either
side of them, so

    The blocker is P`1`[policy] and it is not in the object.

    [policy]:
      (unfinished

reads `P1policy` at `196ecd1a` and carries no severity. Round 5 answered that by transcribing
`markdown-it-py` 3.0.0's `reference` rule -- `parseLinkDestination`, `validateLink`, the title and
its rollback -- **and then put each shape to GITHUB, which is the renderer that renders these
comments.** Executed against `POST /markdown` (`mode: gfm`) on 2026-09-21, over
`[VERDICT][policy]:` beside `[policy]:` and each destination:

| destination | GitHub shows | `markdown-it-py` shows |
|---|---|---|
| `https://example.invalid/x` | `VERDICT:` | `VERDICT:` |
| `<>` | `VERDICT:` | `VERDICT:` |
| a title after it | `VERDICT:` | `VERDICT:` |
| wrapped onto the next line | `VERDICT:` | `VERDICT:` |
| `(unfinished` | **`VERDICT:`** | `[VERDICT][policy]:` |
| `javascript:alert(1)` | **`VERDICT:`** | `[VERDICT][policy]:` |
| rubbish after the destination | `[VERDICT][policy]:` | `[VERDICT][policy]:` |
| two blank lines before it | `[VERDICT][policy]:` | `[VERDICT][policy]:` |

**Two of the eight, and the transcription LOST the correction GitHub shows in both of them** -- a
destination `parseLinkDestination` refuses and a protocol `validateLink` refuses are both
definitions to GitHub, which strips the `href` and shows the label's text all the same. So
`definition_label` reads only what the two agree on -- a label, a colon, and something that is not
whitespace before the block ends -- and `markup_regions` is asked BOTH with reference pairs linked
and with every one of them left written, the way the delimiter runs are asked both ways. **A pair
either renderer LINKS carries its token in the first reading and a pair either renderer SHOWS
carries it in the second**, so being wrong about a definition costs a `manual:` line in one reading
and nothing in the other, and the definition rule stops being load-bearing. `validateLink` is
transcribed nowhere in this file as a result.

**4. GITHUB RENDERS STRIKETHROUGH.** `~~VERDICT~~: CHANGES_REQUIRED` is a correction struck through,
and striking a line out is ordinary writing. It is a GFM extension -- cmark-gfm has it, CommonMark
does not -- so a reading that took `*` and `_` runs and not `~` runs showed a reader the token and
this scan none of it. **Found by round 5's own measurement and not by a lens.**

**5. A COMMENT GITHUB'S EDITOR STORES HAS `\r\n` ENDINGS AND HAD NO BLANK LINES AT ALL.**
`BLANK_LINE` asked for `\n[ \t]*\n`, which no such comment holds, so every rule that stops at a
blank line ran to the end of the comment. **7 of the 684 comments this repository held on
2026-09-21 are stored that way.**

**THIS ONE IS ROUND 5'S OWN, AND IT IS RECORDED AS SUCH.** At `196ecd1a` the inaccuracy cost
nothing that could be found: that head's definition pattern allowed ONE line ending and no more, so
it read no definition whichever endings the comment had, and six shapes chosen to cross a blank
line were executed at `3fe68c37`, at `196ecd1a` and here with both endings without finding one that
answers differently at either of those heads. Item 3 above is what made it cost something:
`definition_label` skips whitespace up to the blank line, so with no blank line a `[policy]:` whose
destination stands TWO blank lines below it became a definition, ``P`1`[policy]`` became the link
`P1policy`, and the severity went. Executed mid-round, before the push: the same words were
`stray=P1` with `\n` endings and none with `\r\n` ones. Found by round 5's own testing, closed in
round 5, and fixtured as a regression against round 5's intermediate state.

**6. AND THE STRUCTURE PASS WAS QUADRATIC, WHICH IS A DENIAL OF SERVICE ON THE MERGE PATH** rather
than a slow test: the audit parses the review synchronously before it enqueues. Two shapes, one of
them introduced by round 4. `MUT-STRAY-STRUCTURE-PASS-IS-BOUNDED` is the fixture.

| comment | at `3fe68c37` | at `196ecd1a` | here |
|---|--:|--:|--:|
| `'[' * 128000 + 'x' + ']' * 128000`, 256 KB | 22.39s | 29.33s | **0.10s** |
| `[policy]:` then 128,000 spaces, 128 KB | 0.04s | 85.85s | **0.05s** |
| `'[x](' * 32000`, 128 KB | 144.20s | 0.53s | **0.52s** |
| `'[a][' * 32000`, 128 KB | 157.23s | 0.06s | **0.06s** |

The first is closed by `link_extent`'s NAMEABLE -- a label holding an unescaped bracket names
nothing a definition can define, decided in constant time off the last bracket the walk passed --
and the second by parsing the definition instead of matching it with a pattern whose two whitespace
repetitions partitioned the same run. The last two were round 4's and are unchanged.

**AND TWO MORE WAYS TO STAND STILL WERE FOUND IN ROUND 5'S OWN CODE AND IN THE CODE IT INHERITED.**
`copied_parts` yielded empty pieces for ever where a part ran past the end of the view; it walks
forward through both sequences now, and each turn either takes a segment off the walk or yields at
least one character. And `reader_spelling` carried a DEFENSIVE SKIP -- a loop whose body no comment
can enter -- written to stop `edge` falling behind the walk; the skip is gone and the same guarantee
is a `max` that always runs, so the walk cannot go backwards and there is no instruction the
parser's own coverage gate can account for only by turning a branch.

## Failure sequence -- the nine classes that are read WIDER than a renderer reads them

Each costs a `manual:` blocker and a person's attention. None can cost a merge.

    `The class is P*1 in the table.`     stray `P1`, where a reader sees `P*1`
    `` The blocker is `&#80;1` here. ``  stray `P1`, where a reader sees `` `&#80;1` ``
    `` The blocker is `_P1_` here. ``    stray `P1`, where a reader sees `` `_P1_` ``
    a ```` ```text ```` fence holding `_P1_`   stray `P1`, where a reader sees `_P1_`
    `[VERDICT]: https://example.invalid/review-policy`   stray `VERDICT:`, where a reader sees
      NOTHING: that line is a link reference definition and a renderer shows none of it
    `Note [a [b](u) P](v)1 here.`       stray `P1`, where a reader sees `Note [a b P](v)1 here.`:
      links do not nest, so a renderer leaves the OUTER pair written and this does not
    `[VERDICT](javascript:alert(1)): CHANGES_REQUIRED`   stray `VERDICT:`, and `markdown-it-py`
      shows the whole line written where GITHUB shows `VERDICT:` -- it strips the `href` and keeps
      the link's text. This reads it GitHub's way, which is why `validateLink` is transcribed
      nowhere in this file
    `[VERDICT][policy]:` above `[policy]: https://example.invalid/x and rubbish`   stray
      `VERDICT:`, where NEITHER renderer links the pair: `definition_label` reads only the label,
      the colon and something non-blank, and this is the over-read that buys the two rows above it
    an indented code block, or a fenced one, whose lines begin with `>` or a list marker
      stray whatever the block's text holds, where a reader sees the block: `block_prefix` reads
      what a LINE carries and nothing about which block it belongs to

The first four are the readings that leave a code span's delimiters written resolving inside it; the
next two `markup_regions` added in round 3; the seventh is round 3's, was first NAMED in round 4 and
is now a DECISION rather than an omission, because GitHub is the renderer that matters and GitHub
keeps the link's text; the eighth and ninth are round 5's -- the price of reading a reference pair
both ways, and the price of the block-prefix reading. **Two more are round 5's and are the price of
following GitHub rather than CommonMark**: a single `~` run is dropped, which cmark-gfm also strikes
through and `markdown-it-py` leaves written, and the content of a raw HTML BLOCK -- `<div>` ...
`</div>` -- has its escapes and delimiters resolved, where a renderer passes it through untouched.

**The code-span rows are not new and predate this file**: `decoded_spelling` has always resolved
JSON escapes over the whole comment, so `` `P1` `` and the same text in a `text` fence have reported
`stray=P1` since long before any of these readings were added -- measured at `a5bcc998` and at
`db94a8ba` alike. **#310 round 1 claimed the opposite** -- that reader and scan agree inside a code
span or a code block -- and that claim was wrong when it was written.

## Measured

Executed at `196ecd1a6f0932126251cc7aa1dae5de2e0d260a` (round 4's head) and at round 5's head, with
`python3` 3.12.3 running each tree's own `scripts/pr-review-parse.py review`. Each comment carries
the prose shown and then one clean `PASS` verdict object. The rendering is `markdown-it-py` 3.0.0's,
with `strikethrough` enabled where the row needs it, taken as the text a reader sees with the tags
stripped.

| prose the comment stores | what a reader sees | at `196ecd1a` | at this head |
|---|---|:-:|:-:|
| `> [VERDICT](url` then `> "Review"): ...` | `VERDICT: CHANGES_REQUIRED ...` | none | **VERDICT:** |
| `> [VERDICT][policy]: ...` with `> [policy]: url` | `VERDICT: CHANGES_REQUIRED ...` | none | **VERDICT:** |
| `> The blocker is P[1][policy].` with `> [policy]: url` | `The blocker is P1.` | none | **P1** |
| `- [VERDICT][policy]: ...` with `- [policy]: url` | `VERDICT: CHANGES_REQUIRED ...` | none | **VERDICT:** |
| `<strong>VERDICT</strong>: ...` | `VERDICT: CHANGES_REQUIRED ...` | none | **VERDICT:** |
| `<code>VERDICT</code>: ...` | `VERDICT: CHANGES_REQUIRED ...` | none | **VERDICT:** |
| `The blocker is P<em>1</em> here.` | `The blocker is P1 here.` | none | **P1** |
| `VER<span>DICT:</span> CHANGES_REQUIRED` | `VERDICT: CHANGES_REQUIRED` | none | **VERDICT:** |
| `~~VERDICT~~: CHANGES_REQUIRED ...` | `VERDICT: CHANGES_REQUIRED ...` | none | **VERDICT:** |
| `The blocker is P~~1~~ here.` | `The blocker is P1 here.` | none | **P1** |
| ``P`1`[policy]`` with `[policy]:` and `(unfinished` | `P1policy` on GitHub, `P1[policy]` to `markdown-it-py` | none | **P1** |
| ``P`1`[policy]`` with `[policy]: javascript:alert(1)` | the same pair of answers | none | **P1** |
| ``P`1`[policy]`` with rubbish after the destination | `P1[policy]` in both | none | **P1** |
| ``P`1`[policy]`` with a far destination, `\r\n` endings | `P1[policy]` in both | none | **P1** |
| `[VERDICT][policy]:` with `[policy]:` and `(unfinished` | `VERDICT:` on GitHub | **VERDICT:** | **VERDICT:** |
| `[VERDICT][policy]:` with `[policy]: javascript:alert(1)` | `VERDICT:` on GitHub | **VERDICT:** | **VERDICT:** |
| `Deferred: __&#80;1**and__ the rest of it.` | `Deferred: P1**and the rest...` | none | none |
| `` `VERDICT`: CHANGES_REQUIRED `` | `VERDICT: CHANGES_REQUIRED` | **VERDICT:** | **VERDICT:** |
| `**VERDICT**: CHANGES_REQUIRED` | `VERDICT: CHANGES_REQUIRED` | **VERDICT:** | **VERDICT:** |
| `[VERDICT](url "4) Review"): ...` | `VERDICT: CHANGES_REQUIRED here.` | **VERDICT:** | **VERDICT:** |
| `[VERDICT][policy]:` with `[policy]:` wrapped | `VERDICT: CHANGES_REQUIRED here.` | **VERDICT:** | **VERDICT:** |
| `The class is P*1 in the table.` | `The class is P*1 in the table.` | **P1** | **P1** |
| `` The blocker is `&#80;1` here. `` | ``The blocker is `&#80;1` here.`` | **P1** | **P1** |
| a `text` fence holding `_P1_` | `_P1_` | **P1** | **P1** |
| `[VERDICT]: https://example.invalid/review-policy` | (nothing) | **VERDICT:** | **VERDICT:** |
| `Note [a [b](u) P](v)1 here.` | `Note [a b P](v)1 here.` | **P1** | **P1** |
| `See findings/P1_security-trust_...md for the rest.` | the same | none | none |

**Every row of this table has a fixture of the same shape** in
`.github/scripts/test-pr-ready-audit.sh`, and the rows are numbered from the first data row:
1 to 8 and 11 to 14 under `MUT-STRAY-BLOCK-PREFIX-STRIPPED`, 9 and 10 under
`MUT-STRAY-GITHUB-STRIKETHROUGH`, 15 and 16 under `MUT-STRAY-DEFINITION-IS-THE-RENDERERS`, 18 to 21
under `MUT-STRAY-READS-THE-WRITTEN-SPELLING` and `MUT-STRAY-LINK-EXTENT-IS-THE-RENDERERS`, and 17
and 22 to 27 under this id. Row 14's own fixture is `MUT-STRAY-CRLF-BLANK-LINE`, which asserts the
two line endings read ALIKE rather than asserting a value.

**Every closed row is driven through the WHOLE AUDIT in BOTH review forms**, because a `stray` field
is not the claim that matters: each is MANUAL with zero `gh pr merge` calls here and was READY with
one at `196ecd1a` -- except rows 15 and 16, which were `stray=VERDICT:` at `196ecd1a`, went silent
under this round's first repair, and are MANUAL again here. **The one row still under-read (17) is
driven through the whole audit as READY with one merge call**, for the same reason.

**Differentially tested against `markdown-it-py` 3.0.0, seed 7, on 2026-09-21**, 100,000 random
strings per alphabet. An under-read is a token or a `VERDICT:` line the renderer shows and no
reading holds.

| generator | at `196ecd1a` | at this head |
|---|:-:|:-:|
| round 3's fragments | 5 under-read, 4,351 over-read | 5 under-read, 4,351 over-read |
| the same plus reference definitions and quoted titles | 0 under-read, 5,964 over-read | 0, 5,964 |
| the same plus blockquote markers, list markers and raw HTML tags | **59** under-read, 3,556 over-read | **1**, 3,579 |

**THE THIRD ROW IS THE POINT AND IT IS WHY EACH ROUND HAS SHIPPED THE NEXT ROUND'S DEFECT.** The
first two alphabets answer IDENTICALLY at a head that reads block prefixes and raw HTML and a head
that does not, because neither can build one: round 4's differential could not have found any of
round 5's five. The third is the same generator with those fragments added, and the 58 under-reads
it closes are exactly the classes above. Over-read rises by 23 in 100,000 in the third row, which is
the loose definition rule reading a pair as a link that `markdown-it-py` shows.

**AND THE INSTRUMENT ABOVE IS NOT THE RENDERER THAT RENDERS THESE COMMENTS**, which round 5 learned
the hard way twice. So the readings were also put to GITHUB, over every comment this repository
holds: `gh api -H 'Accept: application/vnd.github.html+json'` returns a `body_html` for each, which
is GitHub's own rendering, and reducing it to text the way the differential reduces
`markdown-it-py`'s gives what a reader sees. Over the **684** comments, comparing the severity and
MUST tokens GitHub shows against the tokens `stray_summary` reports:

| | at `196ecd1a` | at this head |
|---|:-:|:-:|
| GitHub shows a token in | 378 comments | 378 comments |
| under-read (GitHub shows, no reading holds) | **0** | **0** |
| over-read (a reading holds, GitHub shows none) | **0** | **0** |

`VERDICT:` is left out of that comparison and cannot be put in it: `stray_summary` reports a verdict
line the comment does NOT write, every prose review writes two of its own, and the renderer has no
way to say which is which.

**Round 5's generator cannot build a GFM strikethrough run, a `\r\n` line ending, or a GFM table**,
and the first two were found by hand against GitHub's own renderer rather than by it. That is the
question to ask of the next generator, and the answer for this one.

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

**Asserted end to end by `MUT-REVIEWER-ANY-AUTHOR-READ`**: the witness comment written by
`a-contributor` -- a `VERDICT:` line and a `P1`, both as character references -- is `NOT-READY`,
`blockers=no-review`, zero `gh pr merge` calls, and the clean `PASS` object attributed to the same
outsider is READY, enqueued, one merge call once the login predicate is deleted. Both directions
executed.

**The normal-use limb is a judgement, and the four judgements of this kind made here before were
wrong, so this one says exactly what it rests on.** Round 1 argued that every spelling in this
family "splits a token with markup"; that was false of `**VERDICT**:`, which wraps. Round 2 argued
that a code span is only ever read WIDER than a renderer; that was false in the other direction.
Round 3 argued that the four link forms were covered and the residue was delimiter pairing and
nothing else; that was false of a wrapped definition and of a `)` inside a quoted title. **Round 4
argued that the raw-HTML remainder needs a writer to split a word with a tag; that was false of
`<strong>VERDICT</strong>:`, which wraps the label whole and renders identically to `**VERDICT**:`.
Every one of the four was closed in code rather than argued about here, and so was round 4's.**

What is left is ONE class and it is not a construct at all: **two delimiter runs in one sentence
that a renderer answers differently.** It is not a shape a writer produces on purpose and it is not
a shape a reviewer would notice writing; it is the residue of not implementing CommonMark's
emphasis algorithm, and `stray_summary` covers every answer a renderer gives when it answers both
runs the same way. **That is a bound from a measurement and not a judgement about writing**: 1 in
100,000 random strings from a generator deliberately loaded with `*`, `**`, `_`, `__`, `~~`,
`&#80;` and `VERDICT:`, 0 with those runs removed from the generator, and 0 in the 684 comments
this repository holds. **It should be raised to P1 if a shape of it turns out to be ordinary
writing.**

**And it is not a claim that nothing else is missed.** The class searched for is *an inline
construct GitHub renders that changes which characters a reader sees*, and round 5 searched it by
reading GITHUB's renderer rather than CommonMark: the GFM extensions are strikethrough (closed),
autolink literals (the text is preserved), tables (a `|` hides nothing), task lists and footnotes
(neither deletes characters inside a word). An autolink (`<https://example.invalid/P1>`) keeps its
text and is read. An image (`P![1](...)`) puts the `1` in an `alt` attribute, so a reader does not
see `P1` either. **A different lens would be a different class**, and if one turns up that ordinary
writing produces, this row is a P1 again.

**AND THE WAY THE LAST THREE ROUNDS' DEFECTS WERE ACTUALLY FOUND IS WORTH RECORDING**, because it
was not a lens and it was not a random differential either of the first two times:

- the label fold was closed by putting `folded_label` beside `normalizeReference` over EVERY CODE
  POINT -- the whole domain of one transcribed rule against its source;
- `html_tag_extent` was checked the same way against that renderer's own `HTML_TAG_RE`, over its
  rule's two preconditions, 56 fixed shapes and 200,000 random tag-shaped strings: 0 differences;
- strikethrough, the `\r\n` blank line and the two destinations GitHub links were found by asking
  what GITHUB renders and what GITHUB stores -- not by asking what CommonMark says, and not by
  asking `markdown-it-py`. **THAT IS THE ONE THAT MATTERED MOST**: round 5 transcribed a renderer's
  `reference` rule, confirmed it against that renderer, and would have shipped an under-read in two
  destination shapes, because the renderer it confirmed against is not the one that renders these
  comments.

**What bounds the wider readings to P2 is different and simpler**: each sends a review to a person,
and none can produce a merge. The cost they carry is attention, and round 5 moved it by nothing in
either corpus it can be measured over. **684 real comments** -- every issue comment
`repos/sourcemaps/upstroke/issues/comments` returned on 2026-09-21T18:19:27Z, every one by the
trusted reviewer, 683 of them created before `196ecd1a` was committed and one after: **0 change
outcome** between `196ecd1a` and this head, and **0 gain or lose a stray token**, although 250 of
them READ differently in the two structure readings and 71 in the emphasis one. **208 documents** --
every distinct one `3fe68c37`'s own gate hands to `pr-review-parse.py review`, captured by running
that gate with a `python3` that copies each document it parses -- **1 changes outcome**, and it is
the gate's own raw-HTML fixture, which this round closed on purpose. The harness was controlled both
ways: with round 5's witnesses injected into each corpus, exactly those witnesses are reported
changed and nothing else.

**The 681-comment figure in the previous revision of this file was a stale snapshot** and #310's
round-4 review said so: the API returned 683 created before that head. The number here is the
fetch's own count at the timestamp above, and it will be larger again by the time this is read.

## What the change that takes this up should do

**Decide whether this program pairs delimiters.** That is the one thing left, and it is not another
grammar to transcribe: it is the whole of CommonMark's emphasis algorithm, which is where a reading
of characters stops being one.

- **ASK GITHUB WHAT IT SHOWS, WHICH IS THE SMALLEST DESIGN OF ALL AND WAS NOT NOTICED UNTIL ROUND
  5.** The audit already fetches the comment by id
  (`scripts/pr-ready-audit.sh:974`); the SAME call returns GitHub's own rendering beside the body
  when it is made with `Accept: application/vnd.github.html+json`, so there is no extra round trip
  and no renderer to choose. Executed on 2026-09-21 over all 684 comments this repository holds:
  the call returns a `body_html` for every one. What would be left is reducing HTML to text --
  strip tags, decode entities, know which tags are block-level -- which is a smaller and far more
  checkable job than transcribing CommonMark and GFM, and it is EXACT rather than approximate.
  Measured against these readings over the same 684: 0 under-read and 0 over-read, so it would
  change no outcome today and would have closed every one of the five defects round 5 found
  without transcribing anything. Its cost is that the audit must REFUSE (MANUAL) when the call
  fails, rather than quietly reading the stored text, and that a comment's rendering can change
  when GitHub's renderer does -- which is the behaviour you want here.
- **Render the comment locally.** Complete, and it puts a Markdown implementation and a choice of
  renderer between the review and the merge decision. "What a reader sees" is not one document:
  measured for #286 round 4, `markdown-it-py` 3.0.0 and cmark-gfm 0.29.0.gfm.6 disagree about a
  fence tagged `json` followed by U+0085, U+000B or U+00A0, and round 5 measured two destination
  shapes where following `markdown-it-py` is following the wrong renderer.
- **Refuse prose the program cannot read plainly, or send it to a person.** Fail-closed, and it
  closes every hiding shape at once with no Markdown reading at all. **Measured over the 684
  comments this repository holds:** 674 of them (98.5%) carry at least one of `` ` ``, `[`, `]`,
  `*`, `_`, `&`, `\`, `<` or `~`, and **462 of them (67.5%) carry one of those AND a `P0`-`P3`,
  `MUST` or `VERDICT` somewhere**, which is the narrowest form of the rule that still closes the
  class. Two thirds of this repository's reviews in front of a person is not a cost anyone has
  agreed to pay, and that is what makes this the fallback rather than the design. It remains a
  product decision and it has not been made.
- **Implement the pairing.** CommonMark's emphasis algorithm over the delimiter runs this already
  scans, which would close the one remaining under-read and let the unpaired reading be dropped.
  It is the largest single step this file has left, and the measurement says it buys 1 in 100,000.
- **Narrow the eleven wider readings**, which closes nothing that costs a merge and is worth doing
  only if the `manual:` lines they cost turn out to be paid. They cost none today: 0 of 684, and 0
  against GitHub's own rendering of the same 684.

**AND WHATEVER IS TAKEN, RUN THE DIFFERENTIAL WITH FRAGMENTS THAT CAN BUILD IT, AND AGAINST THE
RENDERER THAT ACTUALLY RENDERS THE COMMENT.** Five rounds of this finding have now been closed by a
review and not by a test; the differential that could have found rounds 3 and 4's defects was run
with a generator whose fragments could not spell them, and the two defects round 5 found itself were
invisible to `markdown-it-py` because GitHub is not `markdown-it-py`.

Whichever is taken, the fixtures under this id in `.github/scripts/test-pr-ready-audit.sh` move with
it, and `findings/PROCESS.md`'s rule applies: the row says which direction was chosen.
