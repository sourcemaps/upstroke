---
id: PR286-PROSE-SCANS-CANNOT-SEE-WHAT-A-READER-SEES
severity: P2
disposition: deferred
category: security-trust
pr: 286
reviewed_sha: 3fe68c37e00beb5a5502f861a2a71be72f619da2
location: scripts/pr-review-parse.py:1052

provenance: pre_existing
first_bad:
guard: a change that decides how this program reads a comment's inline constructs, not only its characters
---

> Found by #286's review at `553cfddf2cee8d36115414372e0d0763be3d9cad`, as the P1
> `PR286-PROSE-SCANS-READ-THE-WRITTEN-SPELLING`. Filed in #310 round 1 as the whole divergence, with
> a severity argument that #310's round-1 review disproved by execution; rewritten at `a5bcc998` to
> what round 2 left, and again at `d599216669fd8e9e079f857a68ae3199a2222fd3`, whose three reviews
> found that a CODE SPAN and a LINK wrapping a label whole were still a merge and that the frontier
> form never got the contradiction check at all -- both closed in round 3.
> **REWRITTEN AGAIN AT `3fe68c37e00beb5a5502f861a2a71be72f619da2`**, whose three reviews found two
> more, BOTH IN THE CODE ROUND 3 ADDED: a reference definition whose destination stands on the line
> after the colon was no definition, so the pair it defines was no link; and the frontier form's
> exemption was a COUNT, so an occurrence a reader never sees cancelled a real correction. Both are
> closed in #310 round 4. Every measurement below was executed against that round's head.

## What this is now

`PROSE_VERDICT` and `stray_summary` read **six** spellings of a review comment: what it stores, what
`json.loads` decodes, and what a reader of the inline prose sees under each combination of the two
questions one reading cannot settle -- whether a renderer PAIRED a delimiter run or left it written,
and whether the comment's code spans and links are CONSUMED or written. `reader_spelling` resolves a
backslash escape, a character reference and an emphasis delimiter run by `markdown-it-py` 3.0.0's
own `escape`, `entity` and `scanDelims`; `markup_regions` reads that renderer's `backtick` rule and
CommonMark's four link forms, with a reference link a link only where a definition defines its
label -- matched by that renderer's own `normalizeReference` fold -- and with each form's EXTENT
read by its own `parseLinkDestination`, `parseLinkTitle` and label rule.
 Each reading also carries the PARTS it was joined out of, so the
frontier form's exemption for its own verdict lines names the occurrence it exempts.

**It is not a renderer**, and this finding is what it is not, measured. **Two classes can still hide
a token from a reader's eye; six are read wider than a renderer reads them.**

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

**What was closed in #310 round 4** is the two ways round 3's own code left the same sentence false,
each READY with one `gh pr merge` call at `3fe68c37` and each MANUAL with none here: a REFERENCE
DEFINITION whose destination stands on the line after the colon (`[VERDICT][policy]:` with
`[policy]:` wrapped, and `The blocker is P[1][policy].`), a `)` INSIDE A QUOTED TITLE ending the
link early (`[VERDICT](url "4) Review"):`), and the frontier form's exemption COUNTING occurrences
rather than matching them, so that a `VERDICT:` inside a link title -- written, and shown to nobody
-- cancelled an appended `` `VERDICT`: CHANGES_REQUIRED ``. A FOURTH was found by comparing
`folded_label` against `normalizeReference` over every code point rather than by a lens: they
agree everywhere but U+0131 DOTLESS I, which that renderer merges with `I` and `i` and `casefold`
keeps apart, so `[VERDICT][`+U+0131+`]:` beside `[i]: https://example.invalid/p` was a link there
and no link here -- READY with one merge call at `3fe68c37`, MANUAL with none here. It needs a
label spelled two ways that differ only in that character, which is not ordinary writing; it was
closed anyway, because the fix is that renderer's own expression rather than a rule to choose.


## Failure sequence -- the six classes that are read WIDER than a renderer reads them

Each costs a `manual:` blocker and a person's attention. None can cost a merge.

    `The class is P*1 in the table.`     stray `P1`, where a reader sees `P*1`
    `` The blocker is `&#80;1` here. ``  stray `P1`, where a reader sees `` `&#80;1` ``
    `` The blocker is `_P1_` here. ``    stray `P1`, where a reader sees `` `_P1_` ``
    a ```` ```text ```` fence holding `_P1_`   stray `P1`, where a reader sees `_P1_`
    `[VERDICT]: https://example.invalid/review-policy`   stray `VERDICT:`, where a reader sees
      NOTHING: that line is a link reference definition and a renderer shows none of it
    `Note [a [b](u) P](v)1 here.`       stray `P1`, where a reader sees `Note [a b P](v)1 here.`:
      links do not nest, so a renderer leaves the OUTER pair written and this does not
    `[VERDICT](javascript:alert(1)): CHANGES_REQUIRED`   stray `VERDICT:`, where a reader sees the
      whole line written: `inline_link_extent` delimits a destination and does not ask whether a
      renderer would FOLLOW it, and that renderer's `validateLink` refuses `javascript:`,
      `vbscript:`, `file:` and most `data:` destinations and shows the brackets instead

The first four are the readings that leave a code span's delimiters written resolving inside it,
the next two `markup_regions` added in round 3, and the last is round 3's too and is named here for
the first time -- round 4 transcribed that renderer's destination and title rules and did not
transcribe its `validateLink`, so the omission is now a decision rather than an accident. **The
code-span row is not wholly new and predates this file**: `decoded_spelling` has always resolved
JSON escapes over the whole comment, so `` `P1` `` and the same text in a `text` fence have reported
`stray=P1` since long before any of these readings were added -- measured at `a5bcc998` and at
`db94a8ba` alike. **#310 round 1 claimed the opposite** -- that reader and scan agree inside a code
span or a code block -- and that claim was wrong when it was written.

**One class of over-read went away in round 4, and it is the other side of the same transcription.**
`[VERDICT](a b): CHANGES_REQUIRED` is no link to `markdown-it-py` 3.0.0 -- `a` is the destination and
`b` is not a title it will take -- and delimiting the parentheses by balancing them alone read it as
one. Measured: `stray=VERDICT:` at `3fe68c37`, **none** here, and the renderer shows the line whole
at both.

## Measured

Executed at `3fe68c37e00beb5a5502f861a2a71be72f619da2` (round 3's head) and at round 4's head, with
`python3` 3.12.3 running each tree's own `scripts/pr-review-parse.py review`. Each comment carries
the prose shown and then one clean `PASS` verdict object. The rendering is `markdown-it-py` 3.0.0's,
taken as the text a reader sees with the tags stripped. The `d599216` column of the previous
revision of this table is not repeated: that parser has no structure reading at all, and every
`markup_regions` row in it read `none`.

| prose the comment stores | what a reader sees | at `3fe68c37` | at this head |
|---|---|:-:|:-:|
| `VER<span>DICT:</span> CHANGES_REQUIRED` | `VERDICT: CHANGES_REQUIRED` | none | none |
| `Deferred: __&#80;1**and__ the rest of it.` | `Deferred: P1**and the rest of it.` | none | none |
| `` `VERDICT`: CHANGES_REQUIRED `` | `VERDICT: CHANGES_REQUIRED` | **VERDICT:** | **VERDICT:** |
| `[VERDICT](https://example.invalid/p): ...` | `VERDICT: CHANGES_REQUIRED` | **VERDICT:** | **VERDICT:** |
| `VER[DICT:](https://example.invalid/x) ...` | `VERDICT: CHANGES_REQUIRED` | **VERDICT:** | **VERDICT:** |
| `` The blocker is P`1` here. `` | `The blocker is P1 here.` | **P1** | **P1** |
| `The blocker is P[1](https://example.invalid/x) here.` | `The blocker is P1 here.` | **P1** | **P1** |
| `[VERDICT][policy]:` with `[policy]:` wrapped | `VERDICT: CHANGES_REQUIRED here.` | none | **VERDICT:** |
| `The blocker is P[1][policy]` with it wrapped | `The blocker is P1 here.` | none | **P1** |
| `[VERDICT](url "4) Review"): ...` | `VERDICT: CHANGES_REQUIRED here.` | none | **VERDICT:** |
| `[VERDICT][`U+0131`]:` beside `[i]: url` | `VERDICT: CHANGES_REQUIRED here.` | none | **VERDICT:** |

| `The class is P*1 in the table.` | `The class is P*1 in the table.` | **P1** | **P1** |
| `` The blocker is `&#80;1` here. `` | ``The blocker is `&#80;1` here.`` | **P1** | **P1** |
| `` The blocker is `_P1_` here. `` | ``The blocker is `_P1_` here.`` | **P1** | **P1** |
| a `text` fence holding `_P1_` | `_P1_` | **P1** | **P1** |
| `[VERDICT]: https://example.invalid/review-policy` | (nothing) | **VERDICT:** | **VERDICT:** |
| `Note [a [b](u) P](v)1 here.` | `Note [a b P](v)1 here.` | **P1** | **P1** |
| `[VERDICT](javascript:alert(1)): ...` | the whole line, written | **VERDICT:** | **VERDICT:** |
| `[VERDICT](a b): CHANGES_REQUIRED here.` | the whole line, written | **VERDICT:** | none |

**Every row of this table has a fixture of the same shape** in
`.github/scripts/test-pr-ready-audit.sh`: rows 3 to 7 under `MUT-STRAY-READS-THE-WRITTEN-SPELLING`
and rows 8 to 10 under `MUT-STRAY-LINK-EXTENT-IS-THE-RENDERERS`, which differ from the prose above
only in carrying the sentence a reviewer would have written around them, and rows 1, 2 and 12 to 17
under this id, which are the prose above. The last two rows have NO fixture: the first is an
over-read this round chose to leave and the second is one it closed, and neither can cost a merge
in either direction.
 **The two rows that are still under-read are driven through the WHOLE AUDIT as
READY with one merge call each**, because a quiet `stray` field is not that claim. The frontier
form's own witnesses are `MUT-PROSE-VERDICT-UNCHECKED` (the appended `**VERDICT**:` correction, and
the same correction as a code span, a link and a character reference) and
`MUT-PROSE-VERDICT-EXEMPTION-BY-IDENTITY` (the correction a link title's own occurrence cancelled),
each with an ordinary prose review as its control.

**Differentially tested against `markdown-it-py` 3.0.0 over 400,000 random strings** built from
fragments a token can hide in, seed 7, on 2026-09-21. An under-read is a token or a `VERDICT:` line
the renderer shows and no reading holds.

| generator | at `3fe68c37` | at this head |
|---|:-:|:-:|
| round 3's fragments | 12 under-read, 17,861 over-read | 12 under-read, 17,861 over-read |
| the same with every `*` and `_` run removed | 0 under-read, 16,025 over-read | 0, 16,025 |
| round 3's fragments plus `[policy]`, `[VERDICT]`, `[policy]:` on its own line, `"` and `'` | **27** under-read, 23,840 over-read | **3** under-read, 23,906 over-read |

**THE FIRST ROW IS THE POINT AND IT IS WHY ROUND 3 SHIPPED THESE TWO DEFECTS.** That generator has
no fragment that can build a reference definition and none that can build a quoted title, so it
answers IDENTICALLY at a head that reads them and a head that does not: the differential round 3
ran could not have found either P1. The third row is the same generator with those fragments added;
the 24 under-reads it closes are all `VERDICT:`, and the 3 that are left are the delimiter-pairing
residue, unchanged. Over-read rises by 66 in 400,000, which is the definition line itself being
read as the text it is not shown as.

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

**The normal-use limb is a judgement, and the three judgements of this kind made here before were
wrong, so this one says exactly what it rests on.** Round 1 argued that every spelling in this
family "splits a token with markup"; that was false of `**VERDICT**:`, which wraps. Round 2 argued
that a code span is only ever read WIDER than a renderer; that was false in the other direction,
because `` `VERDICT`: `` is read NARROWER -- the label disappears. Round 3 argued that the four link
forms were covered and that the residue was delimiter pairing and nothing else; that was false of a
reference definition whose destination stands on the next line, and of a `)` inside a quoted title.
**All three were closed in code rather than argued about here, and so were the frontier form's
missing check and its counted exemption.**

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

**The mixed-pairing under-read is bounded by measurement rather than by judgement**: 12 in 400,000
random strings drawn from a generator deliberately loaded with `*`, `**`, `_`, `__`, `&#80;` and
`VERDICT:`, 0 with those runs removed, and 0 in the 681 comments this repository holds. It needs a
severity written encoded or split AND two delimiter runs in one sentence that a renderer answers
differently. **It is a P2 on the same footing as the rest of this row and it should be raised if a
shape of it turns out to be ordinary writing.**

**AND THE GENERATOR IS PART OF THE BOUND, WHICH ROUND 4 LEARNED THE HARD WAY.** A differential test
bounds only the shapes its fragments can build. Round 3's could not build a reference definition or
a quoted title, and round 3 read its 0 as evidence about links; the two P1s that round shipped were
both in exactly those shapes. The number above is the bound for THIS generator, named in full
beside it, and the first question to ask of the next one is what it cannot build.

**THE OTHER THING THAT FOUND A HIDDEN MERGE WAS NOT A TEST AT ALL**, and it is worth recording
next to the generator: the label fold was closed by taking each transcribed rule and its source
side by side over the whole of their domain -- `folded_label` against `normalizeReference` for
every code point -- rather than by sampling inputs. Where this file transcribes a rule, that
comparison is available and cheap, and it is the check no lens and no generator ran.


**And it is not a claim that nothing else is missed.** The class searched for was *an inline
construct that deletes characters between the token's letters*, by reading CommonMark's inline rules
against `reader_spelling`; the members found were raw HTML, link syntax and the code span, and the
last two are now read. An autolink (`<https://example.invalid/P1>`) keeps its text and is read; an
image (`P![1](...)`) puts the `1` in an `alt` attribute, so a reader does not see `P1` either. **A
different lens would be a different class**, and if one turns up that ordinary writing produces,
this row is a P1 again.

**What bounds the six WIDER readings to P2 is different and simpler**: each sends a review to a
person, and none can produce a merge. The cost they carry is attention, and round 4 moved it by
nothing at all in either corpus it can be measured over. **681 real comments**, every one by the
trusted reviewer: 0 change outcome between `3fe68c37` and this head. **208 documents**, every
distinct one `3fe68c37`'s own gate hands to `pr-review-parse.py review` -- captured by running that
gate with a `python3` that copies each document it parses -- 0 change outcome. The harness was
controlled both ways: with round 4's six witnesses added to each corpus, exactly those witnesses
are reported changed and nothing else.

## What the change that takes this up should do

**Decide whether this program reads a comment's RAW HTML, and whether it pairs delimiters.** Those
are the two left, and they are not the same kind of problem: the first is another grammar to
transcribe, the second is the whole of CommonMark's emphasis algorithm, which is where a reading of
characters stops being one.

- **Render the comment.** Complete, and it puts a Markdown implementation and a choice of renderer
  between the review and the merge decision. "What a reader sees" is not one document: measured for
  #286 round 4, `markdown-it-py` 3.0.0 and cmark-gfm 0.29.0.gfm.6 disagree about a fence tagged
  `json` followed by U+0085, U+000B or U+00A0.
- **Refuse prose the program cannot read plainly, or send it to a person.** Fail-closed, and it
  closes every hiding shape at once -- raw HTML included -- with no Markdown reading at all. **Now
  measured, over the 681 comments this repository holds:** 671 of them (98.5%) carry at least one
  of `` ` ``, `[`, `]`, `*`, `_`, `&`, `\` or `<`, and **459 of them (67.4%) carry one of those AND
  a `P0`-`P3`, `MUST` or `VERDICT` somewhere**, which is the narrowest form of the rule that still
  closes the class. Two thirds of this repository's reviews in front of a person is not a cost
  anyone has agreed to pay, and that is what makes this the fallback rather than the design. It
  remains a product decision and it has not been made.

- **Transcribe the `html_inline` rule the way `escape`, `entity`, `scanDelims`, `backtick` and the
  link rules were transcribed**, which is the cheapest of the three and closes the one class that
  can still cost a merge. It buys nothing against the pairing residue.
- **Narrow the six wider readings**, which closes nothing that costs a merge and is worth doing
  only if the `manual:` lines they cost turn out to be paid. They cost none today: 0 of 681.

**AND WHATEVER IS TAKEN, RUN THE DIFFERENTIAL WITH FRAGMENTS THAT CAN BUILD IT.** Four rounds of
this finding have now been closed by a review and not by a test, and the one test that could have
found rounds 3 and 4's defects was run with a generator whose fragments could not spell them.

Whichever is taken, the fixtures under this id in `.github/scripts/test-pr-ready-audit.sh` move with
it, and `findings/PROCESS.md`'s rule applies: the row says which direction was chosen.
