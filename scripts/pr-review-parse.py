#!/usr/bin/env python3
"""pr-review-parse.py: the whole of a review, or nothing at all.

    scripts/pr-review-parse.py review [--nul] [--out FILE] COMMENT-BODY-FILE
    scripts/pr-review-parse.py ledger [--nul] [--out FILE] PULL-REQUEST-BODY-FILE

Each subcommand reads one file and has exactly two outcomes. Either it builds the whole result,
writes it, confirms the write, and exits 0; or it writes no result at all, says why on stderr, and
exits non-zero. There is no third outcome and no partial one, and that is the entire point of this
program existing.

WHY THIS IS A PROGRAM AND NOT A SHELL PIPELINE
----------------------------------------------

`scripts/pr-ready-audit.sh` decides whether a pull request may be enqueued for merge. Its review
parsing lived in bash -- greps, an awk, an embedded python printing tab-separated records, and an
`END` record to say the stream was whole -- and seven consecutive rounds of frontier review found
the same defect seven times, each time in a stage nobody had armoured yet:

  * `|| true` on a read, so "this grep failed" and "this grep matched nothing" were one answer;
  * a pipeline under `pipefail`, whose status is its RIGHTMOST non-zero one, so a reader that died
    (2) stood behind a matcher's ordinary "no match" (1);
  * a here-string, which bash spills to a temporary file once it outgrows a pipe buffer -- a file
    it cannot create is a redirection that failed, the command never runs, and the shell returns
    1, which for `grep` is an answer;
  * a redirection that fails before its command runs at all, so the command's own vocabulary
    answered for it;
  * `read`, whose end-of-input and failed-read are one status, so a list that stopped arriving was
    counted as a list that ended;
  * an in-band completeness marker, which the review's own strings could write, and one did;
  * and finally an unchecked `printf`: a protocol write that returned EIO while the function
    around it returned 0, so one finding vanished from a findings list that still announced itself
    complete. READY, and a merge call, out of a review that blocks.

The root cause is one sentence: IN BASH, FAILURE IS REPRESENTABLE AS SUCCESS. A status that must
be remembered, a stream with no end-to-end integrity, `errexit` suspended inside `||`. Armouring
each site closed that site; it could not close the class, because each round's site was a new
SHAPE rather than a new instance of a known one, and a gate can only test shapes someone has
already imagined.

So the review protocol is not represented in bash any more. In this program a failed read raises,
a failed write raises, a malformed field raises, and an unexpected exception raises; nothing has
to remember to check a status, and there is no arrangement of the code that emits half a result
and returns 0. The caller's whole contract is: RUN IT; IF ITS EXIT STATUS IS NOT 0, BLOCK;
OTHERWISE READ ITS OUTPUT FROM THE FILE IT WROTE.

Three rules keep that true.

* **The result is built and rendered entirely in memory, and the single write happens after the
  last check.** A parse that dies has written nothing, so "the caller has a result" and "the parse
  finished" are the same fact rather than two facts joined by a marker.

* **The write is confirmed before the exit status is decided, AND ITS COUNT IS READ.** `write()`
  returning EIO is the defect the seventh round found; `write()` returning a SMALLER NUMBER is the
  same defect without an error, and it is the one the twelfth found -- an unbuffered stdout took
  1,024 bytes of a 3,560-byte result, said so in its return value, and the program exited 0 over a
  findings list with 2,536 bytes missing. Every write here loops on what it was told was written
  until the payload is gone or something raises. The payload is written to a staging file THIS
  INVOCATION CREATED, beside the destination, flushed, `fsync`ed and CLOSED -- each of which raises
  rather than returning a status -- and only then renamed over the destination. A destination file
  therefore never exists in a partial state, whatever the caller does with the exit code. The
  staging name is unique because a fixed one is shared: two parses publishing to one destination
  overlapped on `OUT.part`, and the slower one renamed the faster one's bytes into place under its
  own exit 0.

* **Completeness is not signalled in the data.** It is the exit status, which review content
  cannot reach, and -- for the flat rendering the shell reads -- a field count the shell checks
  against the record count the payload declares. The old protocol's `END` row travelled in the
  same tab-and-newline channel as the data it certified, so a reviewer-supplied `base_sha` holding
  "<a real commit>\\nEND\\t-\\t0" printed a valid-looking completeness marker of its own.

AMBIGUITY REFUSES. That is the rule this program reaches a verdict by, and it replaces the
search that used to reach one. There are two review forms -- the workflow's fenced JSON verdict
and the `<!-- upstroke-frontier-review -->` prose -- and they do not agree about the same review:
a JSON review whose object says CHANGES_REQUIRED and carries a P1 reads, to the prose parser, as
the `VERDICT: PASS` line sitting outside the object with no findings at all. So every question
below is answered by counting rather than by finding, and every count that is not one is a
refusal:

  * the comment holds EXACTLY ONE place a verdict could be read from, or there is no result.
    Zero is not a reason to try the other parser; two is not a reason to pick one of them --
    whether the second is a quoted example, a block inside a blockquote, or a block inside an
    HTML comment;
  * every run of three or more backticks or tildes in the comment was consumed by the structure
    scan as a fence of a block that scan found. One that was not means the comment's block
    structure is NOT what this program thinks it is, and there is no result;
  * and no block the verdict was not read from holds a `json` fence -- at the start of a line or
    behind a blockquote's `>` or a list marker -- or a bare verdict opener. Accounting for every
    run says each one was consumed as a fence; it says nothing about WHICH block it was consumed
    into, and CommonMark suppresses a fence inside a raw-HTML block where this scan does not -- so
    a hidden `` ```text `` line swallowed a real blocking verdict as its content, left every run
    accounted for, and let a `PASS` appended after it stand as the only candidate. What such a
    block holds is material, on the same terms as what lies outside one;
  * nothing but whitespace follows the block the verdict is read from, and no `VERDICT:` line
    stands outside it. Not "nothing this program recognises as a block" -- nothing. That line is
    matched AS THE COMMENT SPELLS IT, and a reviewer correcting a generated review writes
    `**VERDICT**: CHANGES_REQUIRED` as readily as the plain form: the spellings only a READER of
    the comment sees are found by `reader_spelling` and reported as a stray token, so the comment
    reaches a person rather than a merge. Two outcomes, and the exact match gets the harder one;
  * no object the verdict is read from names anything twice, AT ANY DEPTH. `json.loads` keeps
    the LAST occurrence of a repeated name, so an object carrying a `findings` array with a P1 in
    it and then a second `"findings":[]` deserialises to a clean PASS with no findings -- one
    document, two readings, and the reading this program would have taken is the one a reader of
    the comment does not see;
  * and the form is not chosen by which parser gets an answer. A comment carrying the prose
    form's marker is the prose form, and one carrying a verdict block as well is a comment
    claiming to be both, which is a refusal rather than a choice.

Three rounds of review put three more spellings of one fenced block through the recogniser this
file used to trust: one leading space, then `> ` before the fence, then the fence hidden in an
HTML comment. A recogniser made cleverer each time is a recogniser that is wrong in a way nobody
has thought of yet. A count does not have to be clever: what it cannot resolve, it refuses, and a
refusal costs the reviewer one re-post where a missed CHANGES_REQUIRED costs a merged P1.

A REAL COMMONMARK PARSE WOULD BE BETTER, and there is none to use: Python's standard library ships
no Markdown parser, `markdown-it-py` and the rest are dependencies, and this repository takes none
for a gate script. What is here is therefore NOT an approximation of the specification -- two of
those have now failed -- but a structure scan whose every unresolved character is a refusal.

THE TWO RENDERINGS are built from the same validated result:

* JSON (the default) -- for a person, and for the gate to assert against;
* `--nul`, a flat NUL-terminated field sequence -- for the shell, which has no JSON parser and
  would need a second program, and so a second success condition, to get one.

NUL is the separator because the review is text and text has no NUL in it -- and where it does,
this program refuses to emit rather than emitting a field that could forge a record boundary.
`$(...)` drops NUL bytes, so the shell reads this through a file and never a command substitution.

EVERYTHING THE OLD PARSERS DECIDED IS DECIDED HERE, to the character. The rules below are ported
one for one from the shapes those parsers had reached after seven rounds of repair -- the head is
the FIRST marker or nothing, a verdict that fails the whole-token check is carried whole rather
than trimmed to the part that passes, a severity outside P0-P3 is a finding the audit cannot
judge rather than one it ignores -- because a rewrite of the parsing path must change no verdict
it should not change, and the corpus of every frontier review in this repository is parsed before
and after to establish that it changed none.
"""

import collections
import html.entities
import json
import os
import re
import sys
import tempfile
import unicodedata

# ---- what a field may be ------------------------------------------------------------------------

# The one channel rule. Every other rule below is about MEANING; this one is about the separator,
# and it is checked at the moment of emission rather than assumed from the rules that built the
# value. Nothing a review can write may add, remove or forge a record.
NUL = "\0"

# The review forms' own shapes, ported from the parsers this replaces.
#
# THESE RANGES ARE RANGES, which the shell's were not. A range inside a POSIX regex is resolved by
# the LOCALE'S COLLATING ORDER and not by ASCII, so `[0-9a-f]` under en_US.utf8 is not the hex
# alphabet it looks like and `[A-Z_]` is not the upper-case one: `VERDICT: FAILÉPASS` parsed as
# PASS under one locale and as FAIL under `C`, out of one file. The audit's login check answers
# that by writing its set out character by character, and the prose parser answered it by running
# every read under `LC_ALL=C`. Python's ranges are code-point ranges and mean the same thing in
# every environment, so they are written as ranges and the gate still runs each case under every
# locale this machine has.
SHA = re.compile(r"[0-9a-fA-F]{7,40}\Z")
VERDICT_WORD = re.compile(r"[A-Za-z][A-Za-z0-9_-]{0,39}\Z")
SEVERITY = re.compile(r"P[0-3]\Z")
# A field of the protocol holds no control character. This is the old parsers' `field()` rule and
# it is kept as it was: an id or a commit that carries one is not narrowed to "absent", because
# absent is a MANUAL line for a person to read and this is a value that would have written rows.
CONTROL = re.compile(r"[\x00-\x1f\x7f]")

# The severity and MUST tokens a person may have written outside the findings. They are reported
# so the audit can send the review to a person; they are never judged here.
#
# `re.ASCII`, so `\b` is the ASCII word boundary GNU grep uses under `LC_ALL=C` -- the locale the
# prose parser ran every read in. Without it Python's `\b` is Unicode-aware, `é` is a word
# character, and a standalone `P1` written between accented characters stops being a token. The
# safe direction for a stray-token scan is to find MORE of them, because each one is a blocker.
STRAY_TOKEN = re.compile(r"\b(?:P[0-3]|MUST)\b", re.ASCII)

# AND THE SPELLING A DECODER READS, NOT ONLY THE ONE THE COMMENT WRITES. `"severity":"P\u0031"` is
# `P1` to `json.loads`, to GitHub's renderer and to the person reading the comment, and it is
# nothing at all to a regex run over the characters the comment spells it with. Every witness in
# this family carried its blocking severity past this scan that way -- the truncation revival, the
# indented fence, the repeated name, the HTML comment and the fenced object all spell it `P\u0031`
# -- because the scan that exists to catch a severity outside the verdict object could not read the
# only spelling those witnesses use. `\uXXXX` and JSON's two-character escapes are resolved below;
# a doubled backslash is consumed as the one character it is, so `\\u0031` is not read as an escape
# it is not. Each `\uXXXX` is resolved on its own, which is enough for every spelling of an ASCII
# token, and `P0`-`P3` and `MUST` are ASCII.
JSON_ESCAPE = re.compile(r"\\u([0-9a-fA-F]{4})|\\(.)", re.S)
SIMPLE_ESCAPE = {'"': '"', "\\": "\\", "/": "/", "b": "\b", "f": "\f",
                 "n": "\n", "r": "\r", "t": "\t"}

# AND THE LANGUAGE A RENDERER GIVES A FENCE, WHICH IS A FUNCTION OF ITS INFO STRING AND NOT A
# SPELLING OF IT. The scan above resolves what `json.loads` resolves, because that is what reads a
# verdict object; what reads an info string is a renderer, and what it reads out of one is the
# block's LANGUAGE. `&#110;` is `n` there, so ```` ```jso&#110; ```` is a `language-json` block to
# the person reading the comment, and it was no `json` fence at all to a rule that compared the
# characters the comment spells it with: the blocking verdict hidden inside a swallowing block while
# a clean `PASS` stood as the only candidate.
#
# CommonMark does not define that language -- "this spec does not mandate any particular treatment
# of the info string" (https://spec.commonmark.org/0.31.2/#fenced-code-blocks) -- so it is whatever
# a renderer computes, and each reading this file derived from the specification instead was wrong
# somewhere a renderer is not. Resolving every reference the grammar admits and then splitting with
# `str.split()` read `json&#133;x` and `json&#11;x` as `json`, which neither renderer measured below
# gives either tag. Splitting what a reference resolved to at a narrower class than `str.split()`'s
# read ```` ```jso&#110; ```` followed by a literal U+0085 as no `json` fence, and `markdown-it-py`
# 3.0.0 renders it `language-json`. And both took the grammar's digit bounds, seven decimal and six
# hex, where both renderers resolve eight: ```` ```jso&#00000110; ```` is `language-json` to each of
# them and was no `json` fence to either reading.
#
# So what is below is not a reading of the specification. It is `markdown-it-py` 3.0.0's own
# function, transcribed: its fence renderer takes the first `str.split()` word of
# `unescapeAll(info)`, and `unescapeAll` is `INFO_ESCAPE` below -- a backslash escape or a
# reference, in ONE left-to-right pass, so an escaped `\&` starts no reference and nothing a
# reference resolves to is read again -- with a name looked up in the table the standard library
# carries (the one that renderer builds its own from), a number of up to eight digits, and a
# reference to any code point `referable` refuses left exactly as it was written. Measured
# 2026-09-14 over every code point, written literally and as a decimal and a hex reference in and
# around the name; every named reference, with and without its semicolon; each letter of the name
# spelled as a number with up to ten leading zeros; a backslash before every printable ASCII
# character; and two million random tags: `rendered_language` equals `markdown-it-py` 3.0.0's
# language on every one, and cmark-gfm 0.29.0.gfm.6 renders `json` on none of them that
# `markdown-it-py` does not. The second fact is why one renderer's function is enough: its `json`
# is the union of both renderers', which is the refusing direction and no wider than a renderer.
INFO_ESCAPE = re.compile(
    r'\\([!"#$%&\'()*+,\-.\/:;<=>?@[\\\]^_`{|}~])' + "|" + r"&([a-z#][a-z0-9]{1,31});",
    re.IGNORECASE,
)
DECIMAL_REFERENCE = re.compile(r"#([0-9]{1,8})")
HEX_REFERENCE = re.compile(r"#x([a-f0-9]{1,8})", re.IGNORECASE)
NAMED_REFERENCE = {name.rstrip(";"): chars for name, chars in html.entities.html5.items()}

# AND THE SPELLING A READER OF THE COMMENT'S PROSE SEES, which is a THIRD text and the one the two
# prose scans below exist to read. `decoded_spelling` is what `json.loads` reads and
# `rendered_language` is what a renderer reads out of an info string; this is what a renderer
# renders out of INLINE PROSE, and it is the reading a reviewer's own eyes use. A rule that asks
# its question of the characters a comment is stored as, when the person writing it is looking at
# what they resolve to, is comparing two different documents -- and that is not a theory here: the
# trusted reviewer prepending `**VERDICT**: CHANGES_REQUIRED` to a generated `PASS` object, which
# is bold and a correction and nothing else, was READY and one merge call where the same words
# written plainly were a refusal and none.
#
# THREE TRANSFORMATIONS, AND THEY ARE THE ONES ORDINARY WRITING PRODUCES: a backslash escape, a
# character reference, and an emphasis delimiter run around or inside the token. `markdown-it-py`
# 3.0.0's own inline rules, transcribed the way `rendered_language` transcribes its `unescapeAll`:
# `escape`'s ASCII-punctuation set, `entity`'s two patterns and their bounds (SEVEN decimal digits
# and six hex, which is NOT `unescapeAll`'s eight, and an unreferable code point becoming U+FFFD
# rather than staying written), and `scanDelims`. One left-to-right pass, so `\&#58;` starts no
# reference and `\*` is no delimiter, and nothing a reference resolves to is read again.
#
# AND THREE TRANSFORMATIONS ARE NOT ALL OF WHAT A READER SEES, because two ordinary constructs
# WRAP A TOKEN WHOLE rather than respelling it: `` `VERDICT`: `` and
# `[VERDICT](https://example.invalid/policy):` are both `VERDICT:` to a reader and carry none of it
# in the characters this pattern spells. A code span and a link are STRUCTURE -- a span a renderer
# consumes and a span it shows -- and a pattern over characters cannot express either, so
# `markup_regions` reads them in a pass of its own and `reader_spelling` runs this one between its
# regions.
#
# WHAT IS STILL NOT READ ANYWHERE IS INLINE RAW HTML: `VER<span>DICT:</span>` renders as the token
# and is no token in any of the readings. It is left open deliberately, with a fixture, because
# splitting a word with a tag is not ordinary writing the way bold, a correction, a code span and a
# link are -- PR286-PROSE-SCANS-CANNOT-SEE-WHAT-A-READER-SEES under `findings/` states what this
# leaves and what it closes.
INLINE_MARKUP = re.compile(
    r'\\(?P<escape>[!"#$%&\'()*+,\-./:;<=>?@\[\\\]^_`{|}~])'
    r"|&(?P<reference>#[0-9]{1,7}|#x[0-9a-f]{1,6}|[a-z][a-z0-9]{1,31});"
    r"|(?P<run>\*+|_+)",
    re.IGNORECASE,
)
# The two character classes CommonMark's flanking rule asks about, and they are `markdown-it-py`
# 3.0.0's. `isWhiteSpace` is transcribed exactly -- tab, line feed, VERTICAL TAB, form feed,
# carriage return, space and every Zs -- and measured equal to it on every code point.
# `isMdAsciiPunct` is the ASCII punctuation set, transcribed exactly; `isPunctChar` is a generated
# table of the Unicode P categories at the version that renderer was built against, and what is
# here is `unicodedata`'s P AND S categories instead, because a table is not something to copy 3 KB
# of into a gate script. Measured over every code point on 2026-09-21: this is a STRICT SUPERSET --
# 7,994 code points are punctuation here and not there, NONE the other way, and NOT ONE of them is
# ASCII. A superset only ever drops a run this keeps, never keeps one it drops, which is the
# direction that finds more tokens; `MUT-STRAY-FLANKING-SUPERSET` asserts that over the whole
# classification rather than over a sample.
ASCII_PUNCTUATION = frozenset("!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~")
MARKUP_WHITESPACE = frozenset("\t\n\x0b\x0c\r ")

# The two shapes `markup_regions` reads a code span's extent with, and they are that renderer's
# `backtick` rule. A BACKTICK STRING IS A MAXIMAL RUN, which is why equal length is equal text
# there and no lookaround has to say so; a BLANK LINE is the only block boundary any of this
# reading knows, and inline rules run inside one block, so a run before one and a run after it are
# two literal runs rather than a code span.
BACKTICK_RUN = re.compile(r"`+")
BLANK_LINE = re.compile(r"\n[ \t]*\n")
# And the block that makes a reference link a link. A LABEL IS DEFINED OR THE BRACKETS ARE SHOWN,
# and `link_labels` says why reading every pair as a link instead loses a token rather than finding
# one. Up to three spaces of indentation, the label, the colon, and something on the line after it.
LINK_DEFINITION = re.compile(r"^ {0,3}\[((?:[^\[\]\\]|\\.)*)\]:[ \t]*(?=\S)", re.M)

# A finding carrying any of these blocks in every lane (MAINTAINING step 5): the deferring
# implementor's ledger row asserts there is no witness, and a witness the review recorded
# contradicts it.
WITNESS_KEYS = ("witness", "reproduction", "repro", "failing_test", "mutation", "mutation_witness")
# A MUST deviation is fixed whatever its label: a field whose NAME says mandatory/deviation/must_,
# or any string field naming MUST as a word.
MUST_KEY = re.compile(r"(mandatory|deviation|must_)", re.I)
MUST_WORD = re.compile(r"\bMUST\b", re.ASCII)

# THE FORM IS THE COMMENT'S OWN MARKER, not whichever parser gets an answer out of it. The prose
# form writes this marker, every prose review in the repository carries it, and no workflow-form
# review does; a comment carrying it AND a verdict block is a comment claiming to be both forms,
# which is a refusal. "I could not read the JSON, so I will try prose" is how a blocking review
# became a PASS: the object said CHANGES_REQUIRED with a P1 spelled `"P\u0031"`, and the prose
# parser read the `VERDICT: PASS` line sitting outside it with no findings at all.
PROSE_MARKER = re.compile(r"<!-- upstroke-frontier-review")
# The prose form's verdict, looked for OUTSIDE a workflow verdict block. No workflow-form review in
# the repository carries one, and one that did would be a comment saying two different things.
#
# WHAT THIS SCAN IS FOR. It detects THE TRUSTED REVIEWER'S COMMENT CONTRADICTING ITSELF -- a
# verdict object and a verdict line in the one comment -- so that this program refuses instead of
# choosing between them. THE VERDICT OBJECT IS THE AUTHORITY for what a workflow-form review says;
# this scan adds nothing to it and decides no verdict of its own.
#
# THE ORDINARY WAY THAT HAPPENS IS A CORRECTION, and that is why this is not a hypothetical rule.
# A review is generated and posted; the reviewer then reads it, disagrees, and prepends a line
# saying so, leaving the object underneath. Written plainly this scan finds it and the parse
# refuses, which is right: the comment says two things.
#
# AND IT IS MATCHED AS THE COMMENT SPELLS IT, WHICH IS HALF OF THE ANSWER. `**VERDICT**:` is how
# the same correction gets written by anyone who reaches for bold, and it carries no `VERDICT:` at
# all in the stored characters. That was exit 0, PASS, READY and one `gh pr merge` call, measured
# through the whole audit at `a5bcc998`, where the plain spelling was exit 1 and no call --
# ordinary Markdown, the trusted reviewer's own account, and a merge the same words would have
# blocked. The other half is in `stray_summary`: the spellings only a READER sees are looked for in
# `reader_spelling` and REPORTED AS A STRAY TOKEN, so the comment goes in front of a person.
#
# THE TWO OUTCOMES ARE DIFFERENT ON PURPOSE. This pattern is exact, so what it finds is exactly a
# contradiction and a refusal is right. `reader_spelling` drops a delimiter run wherever one could
# open or close emphasis, without pairing it, so it reads a token out of `P*1` where a renderer
# shows `P*1` -- an approximation, in the direction of finding more. A refusal costs the reviewer
# the review; a `manual:` blocker costs a person a look. The approximate reading gets the outcome
# that can be paid.
#
# IT IS NOT A TRUST BOUNDARY, and nothing here should be read as one. `scripts/pr-ready-audit.sh`
# parses exactly one comment and it is one the account named as the trusted reviewer wrote --
# `review_comment_filter`'s login predicate, which .github/scripts/test-pr-ready-audit.sh asserts
# on the filter program itself (MUT-REVIEWER-JQ-INJECTION, MUT-REVIEWER-CASE-MISMATCH) and through
# the whole audit (MUT-REVIEWER-ANY-AUTHOR-READ). An account that can write this line can write
# the object instead and put anything in it. THAT IS WHY THE FIX IS NOT AN ENUMERATION OF
# SPELLINGS: what was wrong was not that an attacker could hide a line, it was that the reviewer's
# own ordinary writing did.
#
# AND IT IS ASKED OF BOTH FORMS, DIFFERENTLY. The workflow form writes no verdict LINE of its own,
# so any a reader sees outside its object is reported. The FRONTIER form's verdict IS such a line
# and its template writes two of them, so what is reported there is a line A READER SEES AND THE
# COMMENT DOES NOT WRITE -- `stray_summary` carries the count that says so, and the measurement
# over this repository's own reviews that made the difference necessary.
#
# WHAT IS STILL NOT READ: INLINE RAW HTML. `VER<span>DICT:</span>` renders as `VERDICT:` and is no
# token in any of the readings, measured with `markdown-it-py` 3.0.0 on 2026-09-21 and pinned as a
# fixture in .github/scripts/test-pr-ready-audit.sh. It is left open because splitting a word with
# a tag is not something ordinary writing does, which is exactly what bold, a character reference,
# a code span and a link are -- and a code span and a link are read, by `markup_regions`, because
# they wrap a LABEL WHOLE and `` `VERDICT`: `` was READY with one merge call while it was not.
# PR286-PROSE-SCANS-CANNOT-SEE-WHAT-A-READER-SEES under `findings/` owns that class, the residue
# of not pairing delimiter runs, and the four places these readings are wider than a renderer.
PROSE_VERDICT = re.compile(r"VERDICT:")

# The older bare form's object opener, with whatever a writer left between the brace and the key.
# `\{\"role_understanding` was this pattern without the `\s*`, and ONE SPACE walked past it: a
# comment holding a complete fenced `PASS` example and then a real `{ "role_understanding":...}`
# object saying CHANGES_REQUIRED read as the example, with the object's `"P\u0031"` invisible to
# the stray scan. The bare form has no fence, so its object runs from the opener to the last `}`.
#
# AND THE KEY IS A JSON STRING, SO IT IS READ AS ONE. `\"role_understanding\"` compared the
# characters the comment spells the name with, and the only reader that object ever has is
# `json.loads`, which compares what they DECODE TO: `{"role_understandin\u0067":...}` opens an
# object whose first key is `role_understanding` to that reader, to GitHub's renderer and to the
# person reading the comment, and was no opener at all to the guard. The candidate count reached
# zero, and zero is the road to the prose parser, which read the `VERDICT: PASS` written under it.
#
# The brace and the quote are STRUCTURE and are matched as characters; what stands between the
# quotes is decoded before it is compared, by the pass `decoded_spelling` already runs over a
# severity. `\s` is wider than the four characters JSON calls whitespace, which finds more openers
# rather than fewer -- the direction every rule in this file is wrong in when it is wrong.
OBJECT_OPEN = re.compile(r"\{\s*\"")
# One JSON string's body: every character up to the first quote no backslash introduces. `\\.`
# consumes an escape whole, so the quote inside `\"` does not end the string, and `re.S` lets it
# consume a backslash before a newline. That reads a longer string than JSON does -- JSON allows no
# raw control character in one -- which can only offer an opener JSON has none at, and an opener
# whose object `json.loads` then refuses is a refusal rather than a reading.
JSON_STRING_BODY = re.compile(r'(?:[^"\\]|\\.)*', re.S)
# The name that opener carries, compared after decoding rather than matched before it.
BARE_OBJECT_KEY = "role_understanding"

# A FENCE IS A LINE, AND THE LINES ARE READ IN ORDER, BY COMMONMARK'S RULES -- which are the rules
# GitHub renders these comments by, so they are the rules that decide what a reader of the comment
# sees as the verdict.
#
#   * an opening fence is three or more backticks, or three or more tildes, INDENTED BY UP TO THREE
#     SPACES, followed by an info string naming the language (a backtick fence's info string holds
#     no backtick);
#   * a closing fence is the same character, AT LEAST AS LONG, indented by up to three spaces
#     INDEPENDENTLY OF THE OPENING FENCE, and holds nothing after it but spaces and tabs -- so a
#     content line opening ```` ```a code span``` ```` closes nothing, which is how a
#     `failure_sequence` quoting a code span stays inside its own block. A carriage return is
#     allowed there too: a comment body GitHub stored with CRLF endings is the same comment, and
#     the fence the reader saw closed the block;
#   * the opening fence's indentation is stripped from each line of the content;
#   * a fence that is never closed runs to the end of the comment, and is NOT closed.
#
# https://spec.commonmark.org/0.31.2/#fenced-code-blocks. `^```json` was none of this: one leading
# space and the real verdict stopped being a block at all, so the block before it -- a `PASS` the
# comment quoted as an example -- was selected instead, with the real object's escaped
# `"severity":"P\u0031"` invisible to the stray scan. That is the same revival round 11 closed for
# a truncated block, arriving through the recogniser instead of through the completeness check,
# which is why the rule below does not rest on the recogniser being right.
FENCE_OPEN = re.compile(r"( {0,3})(`{3,}|~{3,})([^\n]*)\Z")
FENCE_CLOSE = re.compile(r" {0,3}(`{3,}|~{3,})[ \t\r]*\Z")
# WHAT A FENCE IS MADE OF, ASKED IN CHARACTERS. Three or more backticks or tildes is the only way
# CommonMark opens a fenced block, so this run is present wherever a fence is -- whatever the rest
# of the line looks like: indented past three spaces, behind a blockquote's `> `, behind a list
# marker, inside an HTML comment. It is what the structure scan is CHECKED AGAINST rather than
# trusted about: every one of these runs must have been consumed by that scan as a fence of a block
# it found, and one that was not means the comment's block structure is not what this program
# thinks it is. That check cannot be written in the recogniser's own vocabulary, or it agrees with
# the recogniser by construction -- which is how a tail check enumerating
# `\{\"role_understanding` missed `{ "role_understanding` and let an earlier PASS stand as the
# verdict.
FENCE_RUN = re.compile(r"`{3,}|~{3,}")
# AND A FENCE RUN INSIDE A BLOCK'S CONTENT THAT WOULD OPEN A `json` BLOCK, asked the same way. A
# fence run this scan consumed AS A FENCE is accounted for by `FENCE_RUN` above whether or not it
# opened the block the reader sees -- which is the hole `unresolved_material` was blind to, because
# accounting for every run says nothing about which block each one belongs to. CommonMark
# suppresses a fence inside a raw-HTML block -- an unclosed `<!--`, a `<div>`, a `<pre>`, any
# complete tag on a line of its own (https://spec.commonmark.org/0.31.2/#html-blocks) -- and this
# scan does not, so a fence opened inside one swallows the real verdict as its content and leaves
# every run accounted for. What such a block's content holds is therefore material too, and a fence
# run whose info string names `json` is the material that matters: that is the one shape the
# verdict is ever read from, so a block whose content carries one is a block this program cannot be
# sure it is reading past.
#
# EVERY RUN, WHEREVER IT STANDS ON ITS LINE -- asked in characters for the reason `FENCE_RUN` is.
# This was a pattern anchored at the start of a line, and `> ```json` walked past it: a `json` fence
# inside a blockquote, which CommonMark renders as the verdict, swallowed by a hidden `text` block
# whose content this rule then read as clean -- the blockquote getting past a content rule the way
# it once got past the recogniser. A list marker opens a fence the same way, and so does a list
# inside a blockquote, so the prefix is not enumerated: the run is found wherever it is and the
# rest of its line is read the way `names_json` reads every other info string. A run this
# matches and CommonMark would not read as a fence -- after prose, indented into code, a backtick
# run whose info string holds a backtick -- is a refusal rather than a reading, which is the
# direction every rule in this file is wrong in when it is wrong.
CONTENT_FENCE_RUN = re.compile(r"(`{3,}|~{3,})(?=([^\n]*))")

# The prose form, read exactly as the shell read it. `[^ \t\n\r\f\v]` is POSIX `[^[:space:]]` in
# the `C` locale, which is what the greps were given.
NOT_SPACE = r"[^ \t\n\r\f\v]"
# The marker AND THE WHOLE RUN AFTER IT, not the part of that run that looks like a commit: a read
# that matches only what is well formed hands back a listing its own failures have been dropped
# from, and the FIRST line of that listing is then not the first marker in the file. A first
# marker that does not read whole is not a licence to take the second.
HEAD_RUN = re.compile(r"(?:head=|Reviewed head: )" + NOT_SPACE + r"*")
HEAD_WHOLE = re.compile(r"(?:head=|Reviewed head: )([0-9a-f]{40})\Z")
# The same rule for the verdict, and the last one in the file wins.
VERDICT_RUN = re.compile(r"VERDICT:\**:? *" + NOT_SPACE + r"*")
VERDICT_WHOLE = re.compile(r"VERDICT:[*]*:? *([A-Z_]+)[*]*\Z")
NUMBERED_FINDING = re.compile(r"[0-9]+\. \*\*(P[0-3])")

USAGE = "usage: pr-review-parse.py {review|ledger} [--nul] [--out FILE] FILE"


class Unparsed(Exception):
    """The parse cannot be completed. Nothing has been written; nothing will be."""


def read_input(path):
    """The bytes of one input, decoded as UTF-8. Unreadable and undecodable both fail.

    A file the parse cannot read is not an empty file, and bytes that are not UTF-8 are not text
    with the bad bytes dropped: either would be a failure arriving as a smaller answer, which is
    the whole class of defect this program exists to end.
    """
    try:
        with open(path, "rb") as handle:
            data = handle.read()
    except OSError as exc:
        raise Unparsed("cannot read %s: %s" % (path, exc))
    try:
        return data.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise Unparsed("%s is not valid UTF-8: %s" % (path, exc))


def clean(value):
    """VALUE as a stripped string, or None when it is absent or cannot be a field.

    `None`, a container and a boolean are not values a review recorded; a string carrying a
    control character is a value that would have written rows of the protocol's own language, and
    it is refused rather than repaired. This is the old parser's `field()`, unchanged.
    """
    if value is None or isinstance(value, (dict, list, bool)):
        return None
    text = str(value).strip()
    if not text or CONTROL.search(text):
        return None
    return text


def matching(value, pattern):
    """VALUE when it matches PATTERN whole, and None otherwise.

    WHAT DOES NOT VALIDATE WHOLE IS NEVER TRIMMED TO THE PART THAT DOES. `[A-Z_]+$` matches the
    tail of a dirty token and reads `FAILÉPASS` as PASS, which is a way to approve a change;
    a commit field holds a commit or it holds nothing.
    """
    text = clean(value)
    return text if text is not None and pattern.match(text) else None


def present(value):
    """Whether a finding's field is filled in. null, false, "" and empty containers are not."""
    return value not in (None, False, "", [], {}) and str(value).strip() != ""


def decoded_spelling(text):
    """TEXT with JSON's string escapes resolved, so an encoded token is a token to the scan above.

    THE SCAN READS THE SEVERITY THE PARSER READS. `"severity":"P\\u0031"` decodes to `P1` -- for
    `json.loads`, for GitHub's renderer and for the person reading the comment -- and carries no
    `P1` at all for a regex over the characters the comment spells it with. That one gap is what
    every witness in this family used to get a blocking severity past `stray_summary`: the severity
    only exists after JSON decoding, and the scan ran over raw text.

    Resolved the way a JSON decoder resolves them, and in one left-to-right pass, so a doubled
    backslash is consumed as the one character it is: `\\\\u0031` is a backslash followed by
    `u0031` and is NOT an escape, which a pass that resolved `\\uXXXX` anywhere would read as one.
    An escape this cannot resolve -- `\\uZZZZ` is not four hex digits, and `\\q` is not an escape
    JSON has -- is left exactly as it was written rather than dropped: a token must not be able to
    hide in the gap between what this understands and what it discards.
    """
    def resolved(match):
        point, simple = match.group(1), match.group(2)
        if point is not None:
            return chr(int(point, 16))
        return SIMPLE_ESCAPE.get(simple, match.group(0))
    return JSON_ESCAPE.sub(resolved, text)


def referable(point):
    """Whether a numeric reference to POINT is resolved, rather than left exactly as it was written.

    `markdown-it-py` 3.0.0's `isValidEntityCode`, range for range: a surrogate, a noncharacter, a
    control character other than tab, line feed, form feed and carriage return, and anything past
    the last code point are not resolved. THIS IS WHERE BOTH EARLIER READINGS WERE WRONG IN THE
    REFUSING DIRECTION. U+000B, U+001C-U+001F and U+0085 are whitespace to `str.split()`, and a
    reference to one of them is none of them: `&#11;` stays `&#11;`, so no word ends there, and
    `json&#11;x` is `language-json&#11;x`. cmark-gfm 0.29.0.gfm.6 does resolve those six references
    and keeps the character inside the language it renders, so it names none of those tags `json`
    either.
    """
    return not (0xd800 <= point <= 0xdfff or 0xfdd0 <= point <= 0xfdef
                or (point & 0xffff) in (0xfffe, 0xffff)
                or point <= 0x08 or point == 0x0b or 0x0e <= point <= 0x1f
                or 0x7f <= point <= 0x9f or point > 0x10ffff)


def rendered_language(info):
    """The language a renderer gives a fenced block whose info string is INFO, or "" for none.

    THE SIBLING OF `decoded_spelling`, AND THE SAME SENTENCE. That one reads what `json.loads`
    reads, because a verdict object's only reader is `json.loads`; this one reads what a renderer
    reads, because an info string's readers are the renderer and the person looking at what it
    rendered. A rule that asks its question of the characters a comment is stored as, when its
    consumer asks the same question of what they resolve to, is comparing two different documents.

    ONE READING, SPLIT ONCE: every escape and reference resolved, then the first word, by
    `str.split()`'s whitespace, of what that leaves -- whichever of its characters were written and
    whichever were resolved. Two readings with a boundary each missed the tag that holds one of
    each: `jso&#110;` then a literal U+0085 was no `json` to the written reading, which split at
    the U+0085, nor to the resolved one, which split nowhere, and it is `language-json` to
    `markdown-it-py` 3.0.0. That renderer reads a NUL as U+FFFD before it parses a line, so this
    does too; neither is whitespace and neither is a letter, so it decides nothing.
    """
    def resolved(match):
        if match.group(1):
            return match.group(1)
        name = match.group(2)
        if name in NAMED_REFERENCE:
            return NAMED_REFERENCE[name]
        number = DECIMAL_REFERENCE.fullmatch(name)
        if number is not None:
            point = int(number.group(1))
        else:
            number = HEX_REFERENCE.fullmatch(name)
            point = None if number is None else int(number.group(1), 16)
        if point is not None and referable(point):
            return chr(point)
        return match.group(0)
    words = INFO_ESCAPE.sub(resolved, info.replace("\0", "\ufffd")).split(None, 1)
    return words[0] if words else ""


def markup_whitespace(ch):
    """Whether CH is whitespace to CommonMark's flanking rule: `markdown-it-py` 3.0.0's own set."""
    return ch in MARKUP_WHITESPACE or unicodedata.category(ch) == "Zs"


def markup_punctuation(ch):
    """Whether CH is punctuation to that rule. A superset of the renderer's, measured and stated
    where `ASCII_PUNCTUATION` is defined: wider only outside ASCII, and wider is the direction that
    drops a delimiter run rather than keeping one."""
    return ch in ASCII_PUNCTUATION or unicodedata.category(ch)[0] in "PS"


def emphasis_delimiter(marker, before, after):
    """Whether a run of MARKER between BEFORE and AFTER can open or close emphasis.

    `markdown-it-py` 3.0.0's `scanDelims`, transcribed: left-flanking and right-flanking as
    CommonMark defines them, `*` opening on the first and closing on the second, and `_` carrying
    the extra clause that makes it INTRAWORD-SAFE -- an underscore between two word characters can
    neither open nor close, which is the whole reason this file can read `_P1_` as the severity a
    reader sees and still read `findings/P1_security-trust_...md` as the citation a reader sees.
    That single rule is what separates the two, and it is the reason the boundary in `STRAY_TOKEN`
    is NOT the thing that changed: the token's boundary is a rule about tokens, and which
    underscores a reader sees is a rule about emphasis.

    A RUN THIS CAN OPEN OR CLOSE IS DROPPED WHEREVER IT STANDS, without pairing it with another --
    which is the one place `reader_spelling` is deliberately not a renderer. Pairing is the whole
    of CommonMark's emphasis algorithm, and an unpaired run a renderer leaves written is dropped
    here: `a*b` reads `ab`. That direction finds tokens a reader does not see, every one of which
    costs a `manual:` line and a person's attention and none of which can cost a merge, and it is
    the direction this file is wrong in everywhere else it is wrong.
    """
    last_ws, next_ws = markup_whitespace(before), markup_whitespace(after)
    last_punct, next_punct = markup_punctuation(before), markup_punctuation(after)
    left = not (next_ws or (next_punct and not (last_ws or last_punct)))
    right = not (last_ws or (last_punct and not (next_ws or next_punct)))
    if marker == "*":
        return left or right
    return ((left and (not right or last_punct))
            or (right and (not left or next_punct)))


def code_span_closer(text, start, ticks):
    """The backtick string that closes a code span opened by TICKS, searching TEXT from START.

    `markdown-it-py` 3.0.0's `backtick` rule: a code span runs from one backtick string to the NEXT
    ONE OF THE SAME LENGTH, and a run with no such closer is that many literal backticks. A
    delimiter nothing closes is written, and it is written here too. `BACKTICK_RUN` matches maximal
    runs only, so "the same length" is the same text and no lookaround has to say so.

    AND IT DOES NOT CROSS A BLANK LINE, because inline rules run inside ONE BLOCK and this is the
    only block boundary this reading knows. A heading or a list that interrupts a paragraph ends
    one too, so a run can pair across one here where a renderer leaves both written -- the same
    over-reading direction as the rest of this reading, and the reason it is a reading BESIDE the
    two that leave every delimiter written rather than instead of them.
    """
    stop = BLANK_LINE.search(text, start)
    limit = len(text) if stop is None else stop.start()
    for run in BACKTICK_RUN.finditer(text, start, limit):
        if run.group(0) == ticks:
            return run
    return None


def code_span_text(content):
    """CONTENT as a code span shows it: CommonMark 0.31.2 6.1, which is that renderer's own rule.

    A line ending inside a code span is a space, and one space comes off each end when both ends
    carry one and the content is not spaces all the way through. NOTHING ELSE HAPPENS TO IT: a
    backslash escape, a character reference and an emphasis run inside a code span are shown
    exactly as they were written, which is why the caller copies this out rather than reading it.
    """
    shown = content.replace("\r\n", " ").replace("\n", " ").replace("\r", " ")
    if len(shown) >= 2 and shown[0] == " " and shown[-1] == " " and shown.strip(" "):
        shown = shown[1:-1]
    return shown


def balanced_run(text, at, opener, closer):
    """The offset just past the CLOSER that balances the OPENER at AT, or None.

    A backslash escape is consumed whole, so `\\)` closes nothing, and the run stops at a blank
    line for the reason `code_span_closer` does.
    """
    depth = 0
    stop = BLANK_LINE.search(text, at)
    limit = len(text) if stop is None else stop.start()
    position = at
    while position < limit:
        here = text[position]
        if here == "\\":
            position += 2
            continue
        if here == opener:
            depth += 1
        elif here == closer:
            depth -= 1
            if depth == 0:
                return position + 1
        position += 1
    return None


def folded_label(label):
    """LABEL as a renderer matches it: CommonMark 0.31.2 4.7, and `markdown-it-py` 3.0.0 does this.

    Leading and trailing whitespace off, every internal run of it one space, and case folded --
    `[Verdict]` and `[ VERDICT ]` name the same definition.
    """
    return " ".join(label.split()).casefold()


def link_labels(text):
    """The labels a link reference definition in TEXT defines.

    A REFERENCE LINK IS A LINK ONLY IF ITS LABEL IS DEFINED. `[VERDICT][policy]` and a bare
    `[VERDICT]` render as a link when something defines the label and AS THE BRACKETS THEMSELVES
    when nothing does, and which one it is decides whether a reader sees `VERDICT:` or
    `[VERDICT]:`. Reading every bracket pair as a link instead was executed and measured: over
    400,000 random strings against `markdown-it-py` 3.0.0 it HID a token, because dropping the
    brackets of a pair a renderer SHOWS joins the words either side of them. ``:P`1`[a]x`` is the
    whole of it -- that renderer shows `:P1[a]x`, which carries `P1`; with the pair read as a link
    it reads `:P1ax`, which carries nothing.

    AN APPROXIMATION OF THE DEFINITION'S OWN GRAMMAR, and deliberately: the destination and title
    are not parsed and the block context is not read, so this finds a definition where a renderer
    would read an ordinary paragraph line. What that decides is a SHORTCUT pair, never an inline
    `[text](destination)`, and both of its directions cost a `manual:` line rather than the review.
    """
    return {folded_label(found.group(1)) for found in LINK_DEFINITION.finditer(text)}


def link_extent(text, bracket, close, labels):
    """(offset just past the link, whether the bracket pair at BRACKET..CLOSE is one at all).

    WHAT A READER SEES OF A LINK IS ITS TEXT. `[VERDICT](https://example.invalid/policy):` shows
    `VERDICT:` and shows nothing of the destination; `[VERDICT][policy]:` and `[VERDICT]:` show the
    same when LABELS defines the label, and show their brackets when it does not.

    THE INLINE FORM NEEDS NO DEFINITION and the other three do, which is CommonMark's own rule and
    the whole reason `link_labels` exists. Neither the destination nor the label is parsed, only
    DELIMITED: what stands between the parentheses is not checked for being a destination a
    renderer would accept, because the check has to be wrong in one direction or the other and
    refusing the pair is the direction that leaves a token hidden.
    """
    after = close + 1
    if after < len(text) and text[after] == "(":
        found = balanced_run(text, after, "(", ")")
        if found is not None:
            return found, True
    if after < len(text) and text[after] == "[":
        found = balanced_run(text, after, "[", "]")
        if found is not None:
            label = text[after + 1:found - 1]
            if not label.strip():                      # the collapsed form names its own text
                label = text[bracket + 1:close]
            if folded_label(label) in labels:
                return found, True
    return after, folded_label(text[bracket + 1:close]) in labels


def markup_regions(text):
    """The spans of TEXT a renderer reads as inline STRUCTURE, sorted and disjoint. Two kinds:

      * `drop` -- what it consumes and shows none of: a code span's two backtick strings, a link's
        brackets, and the destination or label standing after them;
      * `literal` -- a code span's CONTENT, which it shows exactly as the comment wrote it.

    THIS IS THE ONE THING SPELLING CANNOT REACH. `reader_spelling`'s three transformations each
    respell a token; a code span and a link WRAP ONE WHOLE, and `` `VERDICT`: `` and
    `[VERDICT](https://example.invalid/policy):` are `VERDICT:` to a reader with none of it in the
    characters the comment stores. Each was measured at `d599216` prepended to a generated clean
    `PASS` object: exit 0, PASS, no stray, READY, and ONE `gh pr merge` call, where the plain label
    was exit 1 and no call.

    ONE LEFT-TO-RIGHT PASS, and the order in it is the renderer's: a code span binds tighter than a
    link, so `` [a`]`b] `` is a link whose text carries a code span rather than a link ending at
    the `]` inside it. A backslash escape is not structure and protects what follows it, so
    `` \\` `` opens no code span and `\\[` opens no link -- the same set and the same rule as
    `INLINE_MARKUP`'s own escape alternative, in a pass that runs before it.

    A PAIR THAT IS NO LINK IS LEFT WRITTEN, brackets and all, which is `link_labels`' whole
    subject: a renderer shows `[VERDICT]:` where nothing defines the label, and dropping those
    brackets JOINS the words either side of them and loses a token a reader does see. Nested pairs
    are both read, though, where a renderer leaves the OUTER pair of `[a [b](u) c](v)` written
    because links do not nest -- that one is over-reading, it costs a `manual:` line and cannot
    cost the review, and it is the direction every rule in this file is wrong in when it is wrong.
    """
    labels = link_labels(text)
    regions = []
    opens = []
    position = 0
    while position < len(text):
        here = text[position]
        if here == "\\":
            position += 2 if position + 1 < len(text) \
                and text[position + 1] in ASCII_PUNCTUATION else 1
            continue
        if here == "`":
            opener = BACKTICK_RUN.match(text, position)
            closer = code_span_closer(text, opener.end(), opener.group(0))
            if closer is None:
                position = opener.end()
                continue
            regions.append((opener.start(), opener.end(), "drop"))
            regions.append((opener.end(), closer.start(), "literal"))
            regions.append((closer.start(), closer.end(), "drop"))
            position = closer.end()
            continue
        if here == "[":
            opens.append(position)
            position += 1
            continue
        if here == "]" and opens:
            bracket = opens.pop()
            tail, linked = link_extent(text, bracket, position, labels)
            if not linked:
                position += 1
                continue
            regions.append((bracket, bracket + 1, "drop"))
            regions.append((position, tail, "drop"))
            position = tail
            continue
        position += 1
    regions.sort()
    return regions


def reader_spelling(text, emphasis=True, structure=False):
    """TEXT as a reader of the comment's inline prose sees it, for the markup that can hide a token.

    THE THIRD READING, AND THE SIBLING OF `decoded_spelling` AND `rendered_language`. Each of the
    three answers one consumer's question with that consumer's own function: `json.loads` reads a
    verdict object, a renderer reads an info string, and A PERSON READS THE PROSE. The two scans
    below are about what the reviewer wrote for a person to read, so this is the text they are run
    over as well as the one the comment stores.

    Three transformations, in ONE LEFT-TO-RIGHT PASS over the comment, which is what makes them
    compose the way the renderer composes them rather than in some order chosen here:

      * a backslash before ASCII punctuation is dropped -- `markdown-it-py` 3.0.0's `escape` rule
        and its set. `VERDICT\\: CHANGES_REQUIRED` is `VERDICT:` to a reader;
      * a character reference is resolved -- that renderer's `entity` rule, WHICH IS NOT
        `unescapeAll`: up to seven decimal digits or six hex, a named reference looked up in the
        table exactly as it is written, and a code point `referable` refuses rendered as U+FFFD
        rather than left written. `&#80;1` is `P1` to a reader;
      * a `*` or `_` run that can open or close emphasis is dropped -- that renderer's
        `scanDelims`. `**VERDICT**:` is `VERDICT:` and `_P1_` is `P1` to a reader.

    EMPHASIS=FALSE ASKS FOR THE SAME TEXT WITH THE RUNS LEFT WRITTEN, and it is not an option: it
    is THE OTHER HALF OF A PAIRING THIS DOES NOT DO. A renderer either pairs a run and removes it or
    leaves it written, and `stray_summary` scans both answers because this cannot tell which. The
    half that is easy to forget is the SECOND one, and it is the one that can HIDE a token: a run
    this drops and a renderer keeps takes a word boundary with it, so `U**&#80;0` -- `U**P0` to a
    reader, a `P0` bounded by the asterisk -- reads `UP0` here and is no token at all. Found by
    differential test against `markdown-it-py` 3.0.0 over 400,000 random strings on 2026-09-21: 198
    of them, every one of that shape, and SEVEN once both readings are scanned. The seven are the
    residue of not pairing at all -- two runs in one sentence that the renderer answers
    DIFFERENTLY, which neither reading expresses -- and `Deferred: __&#80;1**and__ the rest of it.`
    is fixtured as one.

    STRUCTURE=TRUE ASKS THE FOURTH QUESTION, and it is the one no spelling reaches: `markup_regions`
    reads the comment's CODE SPANS AND LINKS, so a code span's backtick strings and a link's
    brackets and destination are consumed the way the renderer consumes them and a code span's
    content is copied out EXACTLY AS WRITTEN -- no escape, no reference, no delimiter run resolved
    inside it, because a renderer resolves none of them there. `` `VERDICT`: `` and
    `[VERDICT](https://example.invalid/policy):` are `VERDICT:` only in this reading, and each was
    READY with one merge call at `d599216` before it existed.

    IT IS A READING BESIDE THE OTHERS AND NOT INSTEAD OF THEM, for the reason EMPHASIS=FALSE is:
    consuming a delimiter takes a WORD BOUNDARY with it, so ``P1`x` `` reads `P1x` here and is a
    token only where the delimiters stay written. `stray_summary` scans every combination of the
    two questions, which is four readings out of this function; a token in any of them is reported.
    THE TWO QUESTIONS ARE NOT THE SAME KIND, and that is worth saying plainly: pairing emphasis is
    something this file cannot do, while a code span's extent is something it CAN decide and does.
    Both answers are scanned anyway, because deciding right and deciding differently from the
    renderer look identical from here and only one of them can cost a merge.

    WHAT THIS IS NOT. It is not a renderer and it is not a Markdown parse, and the ways it is not
    are each fixtured rather than described. MOST OF THEM READ A TOKEN THE COMMENT DOES NOT SHOW,
    which costs a person a look and cannot cost a merge: it does not pair emphasis delimiters, so
    `P*1` reads `P1` where a renderer leaves the asterisk written; the readings that leave the
    structure written resolve inside a code span and a code block, where a renderer resolves
    nothing; and `markup_regions` reads a bracket pair as a link whatever follows it. Not pairing
    also leaves a small UNDER-read that scanning both answers does not reach -- two runs in one
    sentence answered differently -- measured at 7 in 400,000 above, and fixtured.
    THE ONE THAT IS STILL THE OTHER DIRECTION IS INLINE RAW HTML: `VER<span>DICT:</span>` leaves
    the token in the rendering and none of it in any reading here. Reading it means reading the
    comment's HTML grammar as well, and it is left open deliberately, because splitting a word with
    a tag is not what writing a sentence produces, and that is exactly what bold, a correction, a
    code span and a link are. PR286-PROSE-SCANS-CANNOT-SEE-WHAT-A-READER-SEES under `findings/`
    carries every class, measured at this head and at the head before it.
    """
    def resolved(match):
        escape = match.group("escape")
        if escape is not None:
            return escape
        reference = match.group("reference")
        if reference is not None:
            if reference[0] != "#":
                # Looked up AS WRITTEN. That renderer's pattern is case-insensitive and its table
                # is not, and the table holds several names in more than one case, so the two
                # disagree only on a spelling it holds in NEITHER: `&amp;` and `&AMP;` are both
                # `&`, and `&Amp;` is no reference and stays the five characters a reader sees.
                # Measured against `markdown-it-py` 3.0.0 on 2026-09-21, all three.
                return NAMED_REFERENCE.get(reference, match.group(0))
            body = reference[1:]
            point = int(body[1:], 16) if body[0] in "xX" else int(body)
            return chr(point) if referable(point) else "�"
        run = match.group("run")
        # The characters either side IN THE COMMENT, which is where `scanDelims` reads them -- that
        # renderer scans delimiters over its own source, so a code span consumed before this one
        # runs does not move them -- and a space for each end of the text, because it treats the
        # start of the run's line as whitespace and every line but the first is preceded by the
        # newline that ends the one before it.
        before = text[match.start() - 1] if match.start() else " "
        after = text[match.end()] if match.end() < len(text) else " "
        if emphasis and emphasis_delimiter(run[0], before, after):
            return ""
        return run
    # WITH NO REGIONS THIS IS `INLINE_MARKUP.sub(resolved, text)` AND NOTHING ELSE, which is what
    # the two readings that leave the structure written must stay: no alternative of that pattern
    # can contain a region's first character, so a region never splits a match and the search below
    # is stopped at the next region only to keep that true by construction rather than by argument.
    regions = markup_regions(text) if structure else []
    pieces = []
    position = 0
    index = 0
    while position < len(text):
        # No alternative of `INLINE_MARKUP` can contain a region's first character, so this skip
        # advances past nothing; it is here so that `edge` cannot fall BEHIND `position` and turn
        # the loop below into one that does not end, whatever a later alternative is made of.
        while index < len(regions) and regions[index][0] < position:
            index += 1
        if index < len(regions) and regions[index][0] == position:
            start, end, kind = regions[index]
            index += 1
            if kind == "literal":
                pieces.append(code_span_text(text[start:end]))
            position = end
            continue
        edge = regions[index][0] if index < len(regions) else len(text)
        match = INLINE_MARKUP.search(text, position, edge)
        if match is None:
            pieces.append(text[position:edge])
            position = edge
            continue
        pieces.append(text[position:match.start()])
        pieces.append(resolved(match))
        position = match.end()
    return "".join(pieces)


def stray_summary(outside, contradicting_verdict=False, verdicts_written=0):
    """The tokens found outside the findings, as one field, or None.

    Sorted and joined exactly as `sort -u | tr '\\n' '/'` joined them: both orders are by code
    point, because `sort` ran under `LC_ALL=C` too.

    SIX SPELLINGS ARE SCANNED -- what the comment writes, what a JSON decoder reads, and WHAT A
    READER OF THE PROSE SEES under each combination of the two questions `reader_spelling` cannot
    settle from one reading: whether a renderer paired a delimiter run or left it written, and
    whether the comment's code spans and links are consumed or written. No two of the six are the
    same text. Reading all of them is the safe direction for this scan, the same direction
    `re.ASCII` is chosen for above: every token it finds sends the review to a person, so one found
    in six spellings costs what one found in one costs, and one found in none is the defect.
    `reader_spelling` is the last four, and it is why `_P1_`, `&#80;1` and `P**1**` -- each `P1` to
    the person who wrote the comment and to the person reading it -- are `P1` here.

    THIS IS A NET, AND WHAT IT CATCHES GOES TO A PERSON. A token found here is reported, and
    `scripts/pr-ready-audit.sh` turns it into a `manual:` blocker; it decides no verdict. THE
    VERDICT OBJECT IS THE AUTHORITY, and a severity written outside it is read by nothing else in
    this program.

    AND THE NET IS WHERE THE CONTRADICTING VERDICT LINE IS CAUGHT TOO, which is the whole of
    CONTRADICTING_VERDICT. `the_verdict_block` refuses a comment whose prose carries a literal
    `VERDICT:` line outside the block its verdict is read from, because such a comment says two
    things. A reader sees that same line in spellings the literal pattern carries none of -- the
    six fixtured in .github/scripts/test-pr-ready-audit.sh are `**VERDICT**:`, `VERDICT&#58;`,
    `VERDICT\\:`, `V*ERDICT:*`, `` `VERDICT`: `` and `[VERDICT](url):` -- and the first of them is
    the trusted reviewer prepending an ordinary bold correction to a generated `PASS`. Those reach
    A PERSON rather than a refusal: the refusal is exact because the literal line is exact, and
    these readings drop a delimiter run a renderer would sometimes have left written, so a spelling
    they read and the comment does not show must cost attention and never the review. REPORTED IN
    THIS FIELD AND NOT AS A NEW ONE, so the shell's field count and its "whole result or nothing"
    reading of this program are untouched.

    VERDICTS_WRITTEN IS HOW MANY VERDICT LINES THE FORM WRITES FOR ITSELF, and it is the whole
    difference between the two callers. What is reported is A LINE A READER SEES AND THE COMMENT
    DOES NOT WRITE -- more of them in some reading than in the characters -- and not the mere
    presence of one:

      * the WORKFLOW form's verdict is an object, so it writes none, and `the_verdict_block` has
        already refused every literal one standing before the block. Zero, which makes this "any
        line a reader sees", exactly as it was when only that form asked;
      * the PROSE form's verdict IS a line, and the form's own template writes TWO -- a `VERDICT:`
        summary near the top and the authoritative one at the end, which `parse_prose_review`
        resolves by taking the LAST. Asking "is there a line a reader sees" of that form reports
        EVERY REVIEW IT HAS EVER POSTED: measured over the 681 comments this repository holds on
        2026-09-21, 242 of the 249 readable prose reviews write exactly two and 6 more write
        three, four or five. Asking "does a reader see MORE than the comment writes" reports none
        of them, and still reports `VERDICT: PASS` with `**VERDICT**: CHANGES_REQUIRED` appended --
        the ordinary bold correction, in the form this program's own reviews arrive in, which was
        exit 0, PASS, READY and ONE MERGE CALL at `d599216`.

    WHAT NEITHER READING REACHES is a token cut in half by inline raw HTML, and what the readings
    that leave the structure written read wider than a renderer is an unpaired delimiter run and
    the inside of a code span or a code block. `reader_spelling` says which is which;
    PR286-PROSE-SCANS-CANNOT-SEE-WHAT-A-READER-SEES under `findings/` measures all of them and
    owns them.
    """
    # FOUR READER'S SPELLINGS, and each is one answer to each of two questions this cannot settle
    # from a single reading: a delimiter run a renderer PAIRS is removed and one it does not is
    # left written, and a structure this CONSUMES takes a word boundary with it where leaving it
    # written keeps one. `reader_spelling`'s own docstring carries the measurement behind both.
    paired = reader_spelling(outside)
    unpaired = reader_spelling(outside, emphasis=False)
    paired_read = reader_spelling(outside, structure=True)
    unpaired_read = reader_spelling(outside, emphasis=False, structure=True)
    readings = (paired, unpaired, paired_read, unpaired_read)
    tokens = set()
    for reading in (outside, decoded_spelling(outside)) + readings:
        tokens |= set(STRAY_TOKEN.findall(reading))
    # THE READER'S SPELLINGS ONLY, for this one, and COUNTED rather than searched. A `VERDICT:` the
    # comment spells literally is either already a refusal or already this form's own verdict by
    # the time this runs, and a JSON escape of one of its letters is not a spelling of it anywhere
    # a person reads: `json.loads` resolves that inside the verdict object, and nothing resolves it
    # in prose, where a reader sees the backslash. Scanning `decoded_spelling` for this would
    # report a line no reader of either ever sees.
    if contradicting_verdict:
        seen = max(len(PROSE_VERDICT.findall(one)) for one in readings)
        if seen > verdicts_written:
            tokens.add("VERDICT:")
    return "/".join(sorted(tokens)) if tokens else None


# ---- the review ---------------------------------------------------------------------------------

# One fenced block of a comment: the character its fence is made of, the info string naming its
# language, its content with the opening fence's indentation removed, THE OFFSET ITS OPENING FENCE
# LINE STARTS AT, the raw span that content occupies in the comment, the offset just past the
# block, and whether it was ever closed. `outer` is what lets the whole block -- its fences as well
# as its content -- be taken out of the comment, so that what is left is exactly the material the
# structure scan did not account for.
Block = collections.namedtuple("Block", "fence info content outer start end after closed")


def finding(one):
    """One finding of the workflow form.

    A finding this cannot judge is reported AS a finding the audit cannot judge -- severity `ERR`
    and a reason -- rather than dropped or raised over. That distinction is deliberate and it is
    the one place where "I could not" is a result rather than a failure: the exit status says
    THERE IS NO COMPLETE RESULT, and a review carrying one unreadable finding among five has a
    complete result that names which one. The audit blocks on every `ERR` row it reads.
    """
    if not isinstance(one, dict):
        return {"severity": "ERR", "id": "unparsed", "flags": 0}
    severity = str(one.get("severity", "")).strip()
    raw_id = one.get("id")
    identifier = clean(raw_id)
    if identifier is None and raw_id is not None and str(raw_id).strip():
        # An id that cannot be a protocol field is not narrowed to "no id": no id is a MANUAL line
        # for a person to read, and this is an id that would have written rows.
        return {"severity": "ERR", "id": "bad-id", "flags": 0}
    if not SEVERITY.match(severity):
        return {"severity": "ERR", "id": "bad-severity:" + (identifier or "-"), "flags": 0}
    witness = any(present(one.get(key)) for key in WITNESS_KEYS)
    must = False
    for key, value in one.items():
        if MUST_KEY.search(str(key)) and present(value):
            must = True
        if isinstance(value, str) and MUST_WORD.search(value):
            must = True
    return {
        "severity": severity,
        "id": identifier,
        "flags": int(witness) + 2 * int(must),
    }


def without_indent(lines, indent):
    """LINES with up to INDENT leading spaces removed from each, as CommonMark strips them."""
    cut = []
    for line in lines:
        taken = 0
        while taken < indent and taken < len(line) and line[taken] == " ":
            taken += 1
        cut.append(line[taken:])
    return "\n".join(cut)


def fenced_blocks(text):
    """Every fenced code block of TEXT, in order, with EVERY LINE OF THE COMMENT ACCOUNTED FOR.

    One pass, left to right, holding one piece of state: the fence that is open. That is what makes
    this a structure rather than a search. A regex that jumps to the last opening fence has no
    opinion about the lines it flew over, so a fence it does not recognise is a fence that is not
    there -- and the block before it becomes "the last block". Here a line is either a fence or it
    is content of the block a fence opened, there is no third thing a line can be, and a fence
    inside an open block is content because that is where the scan has got to.

    Each block is reported as the character its fence is made of, its info string, its content with
    the opening fence's indentation removed, THE RAW SPAN THAT CONTENT OCCUPIES IN THE COMMENT --
    so what is outside the verdict can be taken out of the comment by position rather than by
    matching the block's text back against it, which removes every copy of it and not the one that
    was read -- the offset just past the block, and whether it was ever closed.
    """
    blocks = []
    fence_char = None
    fence_len = indent = 0
    info = ""
    content_from = 0
    outer = 0
    content_start = 0
    lines = text.split("\n")
    offset = 0
    for index, line in enumerate(lines):
        line_start = offset
        offset += len(line) + 1
        if fence_char is None:
            opening = FENCE_OPEN.match(line)
            if opening is None:
                continue
            if opening.group(2)[0] == "`" and "`" in opening.group(3):
                continue          # a backtick fence's info string may hold no backtick
            indent = len(opening.group(1))
            fence_char = opening.group(2)[0]
            fence_len = len(opening.group(2))
            info = opening.group(3).strip()
            content_from = index + 1
            outer = line_start
            content_start = line_start + len(line)
            continue
        closing = FENCE_CLOSE.match(line)
        if (closing is not None and closing.group(1)[0] == fence_char
                and len(closing.group(1)) >= fence_len):
            blocks.append(
                Block(fence_char, info, without_indent(lines[content_from:index], indent),
                      outer, content_start, line_start, offset, True)
            )
            fence_char = None
    if fence_char is not None:
        blocks.append(
            Block(fence_char, info, without_indent(lines[content_from:], indent),
                  outer, content_start, len(text), len(text), False)
        )
    return blocks


def outside_block(text, block):
    """TEXT with the verdict block's content taken out of it, for the stray-token scan.

    BY POSITION, not by matching the block's text back against the comment: `text.replace(block,
    "")` removes EVERY copy of that text and not the one that was read, and a block whose content
    was de-indented is no longer a substring of the comment at all, so a review that had its fence
    indented would have had its whole object scanned for stray severities. The span is narrowed the
    way `strip()` would have narrowed the text, so what stays behind is what stayed behind before.
    """
    start, end = block.start, block.end
    raw = text[start:end]
    lead = len(raw) - len(raw.lstrip())
    trail = len(raw) - len(raw.rstrip())
    return text[:start + lead] + text[end - trail:]


def names_json(info):
    """Whether an info string names the verdict's language: the language rendered for it, folded.

    WHAT A RENDERER READS, NOT WHAT THE COMMENT SPELLS. Reading the written spelling is how
    ```` ```jso&#110; ```` -- a `language-json` block to the renderer and to the reader, carrying
    the blocking verdict inside a swallowing block -- was nothing at all to the rule that exists to
    find exactly that.

    Every extra name this finds is a refusal, never a reading: one more candidate makes the count
    two, and a block whose content names `json` is material. The only block it can newly read a
    verdict FROM is one that is the comment's single candidate, whose content `json.loads` must
    then take whole.

    AND ONLY A NAME A RENDERER GIVES. A refusal costs a reviewer the review, and one raised over a
    tag no renderer reads as `json` is a refusal nobody can clear: `json&#133;x` and `json&#11;x`,
    each an ordinary example block beside a valid `PASS`, became "2 places a verdict could be read
    from" under readings that resolved a reference `markdown-it-py` 3.0.0 leaves written.
    """
    return rendered_language(info).lower() == "json"


def spans_outside(text, blocks):
    """The spans of TEXT that no block the structure scan found covers -- FENCE LINES INCLUDED.

    A block's fence lines belong to the block, not to the comment around it, so they are taken out
    with its content. What is left is exactly the material that scan did not account for, and it is
    where everything below looks: a fence run there is a fence this program did not read as one.
    """
    spans = []
    pos = 0
    for one in blocks:
        if one.outer > pos:
            spans.append((pos, one.outer))
        pos = max(pos, one.after)
    spans.append((pos, len(text)))
    return spans


def bare_object_openers(text, start=0, end=None):
    """Every offset in TEXT[START:END] where the older bare form's verdict object opens.

    THE NAME IS COMPARED AS ITS READER DECODES IT. `{"role_understandin\\u0067"` is an object whose
    first key is `role_understanding` to `json.loads` and to everyone looking at the comment, and
    it was not this opener to a pattern that matched the characters instead -- which left the
    candidate count at zero, and zero was the road to the prose parser and the `VERDICT: PASS` line
    written underneath. The brace and the quotes are structure and are read as characters; the name
    between them is a JSON string and is decoded before it is compared.

    The offset returned is the BRACE's, in TEXT as it was given, because the object runs from there
    to the comment's last `}` and that span is what `json.loads` is handed.
    """
    if end is None:
        end = len(text)
    openers = []
    for opener in OBJECT_OPEN.finditer(text, start, end):
        body = JSON_STRING_BODY.match(text, opener.end(), end)
        # An unterminated string is not a key: the body ran to the end of the span this was asked
        # about without a closing quote, so there is no name here to compare.
        if body is None or body.end() >= end or text[body.end()] != '"':
            continue
        if decoded_spelling(body.group(0)) == BARE_OBJECT_KEY:
            openers.append(opener.start())
    return openers


def unresolved_material(text, blocks):
    """Every piece of fence material the structure scan did not turn into a block of its own.

    THIS IS THE CHECK THAT HOLDS WHEN THE RECOGNISER IS WRONG AGAIN, and it is the whole of this
    program's answer to three rounds that each found one more spelling of a fenced block. One
    leading space; then `> ` in front of the fence, which is a fence inside a CommonMark blockquote
    and was no fence at all here; then the fence inside an HTML comment. Each time the recogniser
    was made to understand one more shape, and each time the next round found the next shape.

    So the recogniser is not asked to be right -- it is asked to ACCOUNT FOR EVERY FENCE RUN IN THE
    COMMENT. Three or more backticks or tildes is the only way a fenced block opens, so a run of
    them that the scan did not consume as a fence of some block means this program's idea of where
    the blocks are is not the reader's. There is no result from a comment like that.

    A tilde-fenced `json` block is material for the same reason from the other side: it is a block,
    because a ``` inside one would otherwise be read as a fence of the comment's own, but it is
    never the verdict -- the workflow writes backticks -- so a comment carrying one is a comment
    whose verdict this program cannot be sure it is reading.

    AND WHAT SUCH A BLOCK HOLDS IS MATERIAL, WHICH IS WHERE ACCOUNTING FOR EVERY RUN WAS NOT
    ENOUGH -- the hole `HTML-COMMENT-FENCE-SWALLOWS-VERDICT` and
    `PROSE-CANDIDATE-ZERO-PERMITS-PROSE` both came through. Accounting for every run says each one
    WAS consumed as a fence; it says nothing about WHICH BLOCK it was consumed into. CommonMark
    suppresses a fence inside a raw-HTML block and this scan does not, so `<!--`, a `` ```text ``
    line, `-->`, and then the real verdict, is a `text` block that SWALLOWS that verdict as its
    content -- every run accounted for, the swallowed object invisible to the candidate count, and a
    clean `PASS` appended after it the only candidate left. Measured: the audit reports READY, out
    of a comment a CommonMark parse sees exactly one fenced verdict in, and that one says
    CHANGES_REQUIRED with a P1. An unclosed `<!--` is not the only way; `<div>`, `<pre>` and any
    complete tag on a line of its own start HTML blocks too, so the shape is not a list of tags and
    is not answered by learning one more of them.

    So a block this program did not read the verdict FROM is not inert. Two things inside one are
    material, and they are the two shapes a verdict is ever read from anywhere else in this file:
    a FENCE RUN NAMING `json`, wherever it stands on its line, and the older bare form's OBJECT
    OPENER -- which is the whole of the second finding, whose `text`-fenced `role_understanding`
    object left the candidate count at zero and sent a self-contradictory comment to the prose
    parser. A comment whose `text` block holds either has no result -- which is the same sentence as
    the one above it, asked of the inside of a block rather than of the outside of one. Wherever it
    stands, because a pattern anchored at the start of the line let `> ```json` through: a `json`
    fence inside a blockquote is a verdict to CommonMark, and inside a swallowing block it was
    nothing to this rule.
    """
    stale = []
    for start, end in spans_outside(text, blocks):
        for run in FENCE_RUN.finditer(text, start, end):
            stale.append("%s at line %d" % (run.group(0)[:8], text.count("\n", 0, run.start()) + 1))
    for one in blocks:
        if one.fence == "`" and names_json(one.info):
            # The verdict's own shape. What is inside THIS is read by `json.loads`, which is a
            # whole-document check no fence run can pass: a line-initial run inside a valid JSON
            # object would have to be inside a string, and a JSON string holds no newline -- and a
            # run behind a blockquote's `>` or a list marker is no more JSON than one without.
            continue
        at = text.count("\n", 0, one.outer) + 1
        for run in CONTENT_FENCE_RUN.finditer(one.content):
            if names_json(run.group(2)):
                written = run.group(2).split(None, 1)
                stale.append("%s%s inside the block at line %d"
                             % (run.group(1)[:8], (written[0] if written else "")[:8], at))
        if bare_object_openers(one.content):
            stale.append("a verdict object inside the block at line %d" % at)
        if one.fence != "`" and names_json(one.info):
            stale.append("%s%s at line %d" % (one.fence * 3, one.info[:16], at))
    return stale


def verdict_candidates(text, blocks):
    """Every place in TEXT a workflow verdict could be read from, in the order they appear.

    A candidate is a backtick-fenced block whose info string names `json` -- the shape the review
    workflow writes -- or the older bare form's object, which has no fence and runs from its opener
    to the last `}` in the comment. Both are looked for over the WHOLE comment and not from one end
    of it: the caller requires exactly one, so there is no last, no first, and no earlier block to
    fall back to. An opener inside a block the scan found is that block's content and not a
    candidate, which is why a workflow verdict object -- whose own first key is `role_understanding`
    -- is one candidate rather than two.
    """
    found = [one for one in blocks if one.fence == "`" and names_json(one.info)]
    close = text.rfind("}")
    for start, end in spans_outside(text, blocks):
        for at in bare_object_openers(text, start, end):
            found.append(Block("`", "json", text[at:close + 1], at, at, close + 1, close + 1,
                               close >= at))
    found.sort(key=lambda one: one.outer)
    return found


def the_verdict_block(text, candidates):
    """The ONE block this comment's verdict is read from, or a refusal. There is no third answer.

    EXACTLY ONE, AND NOTHING LOOSE AROUND IT. Round 11 made a block that does not close a failed
    parse rather than a licence to take the block before it; round 12 found the same revival
    arriving through the recogniser, and closed it by taking the comment's LAST block; round 13
    found three more ways to make the last block the wrong one -- a bare object the tail check's
    enumeration missed, a real block hidden from the recogniser inside a blockquote, and a second
    block hidden from a reader inside an HTML comment.

    Taking the last block was still a search. This is not: the comment either holds one place a
    verdict could be read from, or it does not read. A second place is a refusal whether it is a
    quoted example, a blockquoted block or a hidden one -- this program does not have to tell them
    apart, and every attempt to tell them apart has been the defect.

    The two checks after the count are the same rule pointed outwards. Nothing but WHITESPACE may
    follow the block -- not "nothing this program recognises as a block", which is the enumeration
    that failed; and no `VERDICT:` may stand outside it, because a comment carrying a verdict object
    and a verdict line says two things and this program would be choosing between them. That second
    check reads the line AS THE COMMENT SPELLS IT, and it is deliberately the only one of the two
    readings that refuses: the spellings only a reader of the comment sees -- `**VERDICT**:` among
    them, which is what an ordinary correction looks like -- are found by `reader_spelling` and
    sent to a person as a stray token instead. `PROSE_VERDICT` states why the two outcomes differ.
    """
    if len(candidates) != 1:
        raise Unparsed(
            "the review carries %d places a verdict could be read from, and exactly one is read"
            % len(candidates)
        )
    block = candidates[0]
    if not block.closed:
        raise Unparsed("the review's verdict block does not close")
    tail = text[block.after:]
    if tail.strip():
        raise Unparsed(
            "the review carries [%s] after the block its verdict is read from"
            % tail.strip()[:24]
        )
    if PROSE_VERDICT.search(text[:block.outer]) is not None:
        raise Unparsed(
            "the review carries a VERDICT: line outside the block its verdict is read from"
        )
    return block


class RepeatedName(ValueError):
    """A JSON object named the same thing twice, so the document has two readings.

    A `ValueError`, because that is the vocabulary every other unreadable verdict object arrives
    in and the decode below has exactly two outcomes either way: a whole object, or a refusal.
    Subclassing it means a rewrite that stops naming this class by hand still cannot turn a
    repeated name into a result -- the `except ValueError` it falls into is a refusal too.
    """

    def __init__(self, name):
        # The name is REVIEW CONTENT and it is rendered as `repr` rather than as itself: a name
        # holding a newline would otherwise write a second diagnostic line of its own, which is
        # the in-band-marker shape this file exists to keep out of its own channels. `repr`
        # escapes every control character and every lone surrogate, and the length is bounded the
        # way every other quoted fragment here is bounded.
        super().__init__("names %.40r twice" % name)


def one_reading(pairs):
    """One JSON object built from PAIRS -- or a refusal, because A REPEATED NAME IS NOT A VALUE.

    `json.loads` KEEPS THE LAST OCCURRENCE OF A REPEATED NAME
    (https://docs.python.org/3/library/json.html#repeated-names-within-an-object), and that is a
    verdict this program would be choosing rather than reading. One fenced object with a valid
    head and base, `"verdict":"PASS"`, a `findings` array carrying a P1, and then a second
    `"findings":[]`, deserialised to a clean PASS with no findings AT ALL -- the blocking array
    discarded before validation ever saw it, and the stray scan silent because the object is the
    recognised verdict block and its severities are inside it. READY, and a merge call, out of a
    review that blocks. The same shape one level down erases a witness instead: a finding with
    `"witness":"a failing test"` and then `"witness":null` is a finding the audit stops holding
    in every lane.
    """
    seen = set()
    for name, _ in pairs:
        if name in seen:
            raise RepeatedName(name)
        seen.add(name)
    return dict(pairs)


def parse_json_review(text, candidates):
    """The workflow form: the comment's one verdict object, and it is the only source.

    Anything in the comment outside that object which looks like a finding is for a person, and is
    reported as a stray token rather than counted as a finding.
    """
    block = the_verdict_block(text, candidates)
    try:
        verdict = json.loads(block.content, object_pairs_hook=one_reading)
    except RepeatedName as exc:
        # AMBIGUITY REFUSES, and a repeated name is the ambiguity this file's own rule was written
        # about: two readers read two verdicts out of one document. The hook is the object's
        # CONSTRUCTOR, so the refusal is not a check run over a decoded object that could be
        # fooled about what the text said -- the object is never built, at any depth, and there is
        # no value for a later reader to disagree about.
        raise Unparsed("the review's verdict object %s, so it has two readings" % exc)
    except ValueError as exc:
        # THE ONE CANDIDATE IS THE VERDICT, and this one does not read as a whole object. Reaching
        # past it to another block is the defect; reporting it as a review with no findings is the
        # same defect wearing the other hat. There is no complete result here.
        raise Unparsed("the review's verdict object is not whole: %s" % exc)
    if not isinstance(verdict, dict) or not isinstance(verdict.get("findings"), list):
        # The comment carries a verdict block and it holds no verdict object this can read. That
        # is a complete description of the review -- it records nothing, and it carries one finding
        # the audit cannot judge -- so it is a result, and every part of it blocks.
        return {
            "kind": "json",
            "reviewed_sha": None,
            "verdict": None,
            "base_sha": None,
            "stray": stray_summary(text),
            "findings": [{"severity": "ERR", "id": "unparsed", "flags": 0}],
        }
    outside = outside_block(text, block)
    return {
        "kind": "json",
        "reviewed_sha": matching(verdict.get("reviewed_sha"), SHA),
        "verdict": matching(verdict.get("verdict"), VERDICT_WORD),
        "base_sha": matching(verdict.get("base_sha"), SHA),
        # AND THE CONTRADICTING VERDICT LINE IS ASKED FOR HERE WITH NOTHING EXEMPT. This form's
        # verdict is its object's, so it writes no verdict LINE of its own and any a reader sees
        # outside that object is a comment saying two things, which belongs in front of a person.
        # `parse_prose_review` asks the same question with the lines that form does write exempted;
        # `stray_summary` carries why the two callers differ.
        "stray": stray_summary(outside, contradicting_verdict=True),
        "findings": [finding(one) for one in verdict["findings"]],
    }


def parse_prose_review(text):
    """The frontier form, read conservatively: numbered `N. **P<n>` findings and the last VERDICT.

    A severity written any other way -- a heading, a sentence -- is a stray token and sends the
    review to a person. The prose form records no base commit, and says so with a null.

    AND A VERDICT LINE A READER SEES THAT THIS FORM DID NOT WRITE is a stray token too, which is
    this form's half of the contradiction check: `VERDICT: PASS` with an appended
    `**VERDICT**: CHANGES_REQUIRED` is the reviewer correcting their own review, and it was exit 0,
    PASS, READY and one `gh pr merge` call at `d599216` where the plain words were
    CHANGES_REQUIRED and none.
    """
    head = None
    first = HEAD_RUN.search(text)
    if first is not None:
        whole = HEAD_WHOLE.match(first.group(0))
        head = whole.group(1) if whole is not None else None
    verdict = None
    runs = VERDICT_RUN.findall(text)
    if runs:
        whole = VERDICT_WHOLE.match(runs[-1])
        # A CHECK THAT FAILED IS THE ANSWER, NEVER THE INPUT TO ANOTHER ATTEMPT. Not one character
        # is stripped off a token that failed the whole-token check: the run stands exactly as it
        # was read, `VERDICT:` and all, so it is not PASS, cannot be turned into PASS, and names
        # itself where the audit prints the blocker. Salvaging the clean word out of
        # `VERDICT: ::PASS` was a new way to approve a change, minted by the branch that had just
        # rejected it.
        verdict = whole.group(1) if whole is not None else runs[-1]
    numbered = [
        {"severity": found.group(1), "id": None, "flags": 0}
        for found in (NUMBERED_FINDING.match(line) for line in text.split("\n"))
        if found is not None
    ]
    outside = "\n".join(
        line for line in text.split("\n") if NUMBERED_FINDING.match(line) is None
    )
    return {
        "kind": "prose",
        "reviewed_sha": head,
        "verdict": verdict,
        "base_sha": None,
        # AND THE CONTRADICTING VERDICT LINE IS ASKED FOR HERE TOO, WITH THIS FORM'S OWN LINES
        # EXEMPTED. This form's verdict is a `VERDICT:` line and its template writes two of them,
        # so the question the workflow form asks -- is there a line a reader sees -- would report
        # every prose review ever posted. The question asked here is whether A READER SEES MORE OF
        # THEM THAN THE COMMENT WRITES, which is the same rule with the exemption made explicit:
        # `VERDICT: PASS` with `**VERDICT**: CHANGES_REQUIRED` appended is one written and two
        # seen, and that is an ordinary bold correction of a review in the form this program's own
        # reviews arrive in. It was exit 0, PASS, READY and one `gh pr merge` call at `d599216`.
        #
        # COUNTED OVER `OUTSIDE` AND NOT OVER `TEXT`, because that is the text the readings are
        # taken of: a numbered finding line is not scanned, so a `VERDICT:` written on one must not
        # be exempted either.
        "stray": stray_summary(outside, contradicting_verdict=True,
                               verdicts_written=len(PROSE_VERDICT.findall(outside))),
        "findings": numbered,
    }


def review_result(args):
    """review FILE: the one review this pull request is judged on, in whichever form it is in.

    THE ORDER HERE IS THE POINT. The comment's block structure is established first and checked
    against the comment's own characters; only then is a form chosen, and the form is chosen by the
    comment's marker rather than by which parser gets an answer. Every branch that cannot be
    resolved ends in a refusal, and none of them ends in the other parser:

      * material this program could not account for as a block -> no result. A fence run outside
        every block it found, and -- because accounting for every run says nothing about which
        block each one went into -- a fence run naming `json`, wherever it stands on its line, or a
        bare verdict opener INSIDE a block it did not read the verdict from. The structure is not
        what it thinks it is, so nothing built on that structure can be trusted;
      * the prose form's marker AND a verdict block -> no result. The comment claims to be both
        forms, and they do not agree about the same review;
      * the prose form's marker and nothing else -> the prose parser, which is the form the comment
        says it is rather than the one left over;
      * no marker -> the workflow form, which must yield exactly one verdict block. ZERO IS A
        REFUSAL, not a reason to try prose: that fall back is how a real CHANGES_REQUIRED hidden
        from the recogniser -- one leading space, a `> ` before the fence -- became the
        `VERDICT: PASS` line written outside it.
    """
    if len(args) != 1:
        raise Unparsed("review takes one file")
    text = read_input(args[0])
    blocks = fenced_blocks(text)
    stale = unresolved_material(text, blocks)
    if stale:
        raise Unparsed(
            "the review carries material this parse could not account for as a block: [%s]"
            % stale[0]
        )
    candidates = verdict_candidates(text, blocks)
    if PROSE_MARKER.search(text) is not None:
        if candidates:
            raise Unparsed(
                "the review carries the prose form's marker and %d verdict block(s)"
                % len(candidates)
            )
        result = parse_prose_review(text)
    else:
        result = parse_json_review(text, candidates)
    result["tag"] = "review"
    return result


def review_fields(result):
    """The review as flat fields: seven, the last of which is the finding count, then three each.

    The count is what lets the shell tell a payload that arrived whole from one that stopped part
    way -- `read` ends a loop at end of input and at a failed read alike -- and it is computed
    here, from the result, rather than being a marker any content could write.
    """
    fields = [
        "review",
        result["kind"],
        result["reviewed_sha"] or "-",
        result["verdict"] or "-",
        result["base_sha"] or "-",
        result["stray"] or "-",
        str(len(result["findings"])),
    ]
    for one in result["findings"]:
        fields += [one["severity"], one["id"] or "-", str(one["flags"])]
    return fields


# ---- the ledger ---------------------------------------------------------------------------------


LEDGER_HEADING = "## Review finding ledger"
LEDGER_ID_HEADER = re.compile(r" *ID *\Z")
LEDGER_SEPARATOR = re.compile(r"-+\Z")


def ledger_result(args):
    """ledger FILE: the id and disposition of every row of the pull-request body's finding ledger.

    The header row, the separator and the canonical `None yet` row are not rows, exactly as the
    awk this replaces judged them -- on the untrimmed cell, which is why the patterns carry their
    own spaces. Everything else in the section is a row and is reported as one; a cell holding
    something no finding id could equal can neither match a finding nor hide a match, so there is
    nothing here for a malformed row to gain.
    """
    if len(args) != 1:
        raise Unparsed("ledger takes one file")
    rows = []
    in_ledger = False
    for line in read_input(args[0]).split("\n"):
        if line.startswith(LEDGER_HEADING):
            in_ledger = True
            continue
        if line.startswith("## "):
            in_ledger = False
        if not in_ledger or not line.startswith("|"):
            continue
        cells = line.split("|")
        identifier = cells[1] if len(cells) > 1 else ""
        if LEDGER_ID_HEADER.match(identifier) or LEDGER_SEPARATOR.match(identifier):
            continue
        if "None yet" in identifier:
            continue
        rows.append(
            {
                "id": identifier.strip(" "),
                "disposition": (cells[9] if len(cells) > 9 else "").strip(" "),
            }
        )
    return {"tag": "ledger", "rows": rows}


def ledger_fields(result):
    """The ledger as flat fields: two, the second of which is the row count, then two each."""
    fields = ["ledger", str(len(result["rows"]))]
    for row in result["rows"]:
        fields += [row["id"], row["disposition"]]
    return fields


# ---- rendering and the one write ----------------------------------------------------------------

SUBCOMMANDS = {
    "review": (review_result, review_fields),
    "ledger": (ledger_result, ledger_fields),
}


def rendered(subcommand, result, nul):
    """The bytes to write, complete, or a failure before anything is written."""
    if not nul:
        return (json.dumps(result, indent=2, sort_keys=True, ensure_ascii=False) + "\n").encode(
            "utf-8"
        )
    fields = SUBCOMMANDS[subcommand][1](result)
    for one in fields:
        # The last check before a field becomes a record boundary's neighbour. Nothing that
        # reaches here can hold the separator, and this is where that is established rather than
        # assumed from the rules that built it.
        if NUL in one:
            raise Unparsed("refusing to emit a field holding the record separator")
    return "".join(one + NUL for one in fields).encode("utf-8")


def write_all(stream, payload):
    """All of PAYLOAD through STREAM, or a failure. Never some of it and a return.

    `write()` RETURNS A BYTE COUNT, AND IT IS NOT ALWAYS THE WHOLE PAYLOAD. Under
    `PYTHONUNBUFFERED=1` `sys.stdout.buffer` is the raw file object, whose `write` does one
    `write(2)` and hands back what the kernel took: with the caller's `RLIMIT_FSIZE` at 1024 and a
    3,560-byte result, `write(1, ..., 3560) = 1024` -- and the count discarded, a flush of an
    unbuffered stream finishing nothing, the program exited 0 over a payload with 2,536 bytes
    missing. That is the seventh round's unchecked `printf` again: a short write is a failure that
    looks exactly like a success, and it truncates a findings list without shortening the count the
    list declares.

    So the count is read. Every byte is accounted for before this returns, and a write that stops
    making progress raises rather than reporting what it managed.
    """
    view = memoryview(payload)
    while view:
        written = stream.write(view)
        if written is None:
            raise OSError("the stream would not say how many bytes of the result it wrote")
        if written <= 0:
            raise OSError(
                "the result stopped being written with %d of %d bytes left"
                % (len(view), len(payload))
            )
        view = view[written:]


def write_result(payload, out):
    """The single write, confirmed before it counts as one.

    Every step here raises rather than returning a status, and the destination is renamed into
    place only once the bytes are on the device: a `write()` that returns EIO, a `flush()` that
    finds the device full, a `close()` that fails -- each ends the program non-zero with no
    destination file to read. That is the finding this program was written for, and it is closed
    by construction rather than by a check somebody has to remember. The one step that does NOT
    raise is a `write()` that took some of the payload and said so, which is why both writes go
    through `write_all` and neither is the bare `.write()` whose count nobody reads.

    THE STAGING FILE IS CREATED BY THIS INVOCATION AND BELONGS TO IT. `OUT + ".part"` is one
    pathname shared by every process writing to one destination, and two of them overlapping is not
    a theoretical shape: A flushes its blocking review and pauses in `fsync`, B truncates the same
    staging file and writes a clean `PASS` over it, A wakes and renames B's bytes into A's
    destination -- exit 0, publishing a verdict out of an input it never read. `mkstemp` creates
    with `O_EXCL` under a name nothing else holds, in the destination's own directory so the rename
    stays within one filesystem and stays atomic. Standards section 8: a unique staging path, never
    a fixed temporary name concurrent writers can collide on.
    """
    if out is None:
        write_all(sys.stdout.buffer, payload)
        sys.stdout.buffer.flush()
        # Closed explicitly: a buffered write that failed must raise HERE, where the exit status
        # is still being decided, and not in an interpreter shutdown handler after this function
        # has already reported success.
        sys.stdout.close()
        return
    handle_fd, part = tempfile.mkstemp(
        dir=os.path.dirname(out) or os.curdir, prefix=os.path.basename(out) + ".", suffix=".part"
    )
    try:
        with os.fdopen(handle_fd, "wb") as handle:
            write_all(handle, payload)
            handle.flush()
            os.fsync(handle.fileno())
        # `mkstemp` creates 0600, and the destination is what a plain create would have left there.
        # Making the staging path unique is not also a licence to change what the caller reads.
        mask = os.umask(0)
        os.umask(mask)
        os.chmod(part, 0o666 & ~mask)
        os.replace(part, out)
    except OSError:
        try:
            os.unlink(part)
        except OSError:
            pass
        raise


def complain(message):
    """Say why on stderr, through the byte layer: the environment does not choose this encoding.

    `print` would encode through a text layer whose encoding comes from the environment, and
    `PYTHONIOENCODING=ascii` with one non-ASCII character in a finding id is exactly how the
    parser this replaces was made to die halfway through its output.
    """
    sys.stderr.buffer.write(("pr-review-parse: " + message + "\n").encode("utf-8", "replace"))
    sys.stderr.buffer.flush()


def main(argv):
    nul = False
    out = None
    rest = []
    expecting_out = False
    for arg in argv:
        if expecting_out:
            # `--out` will not take the next option as its value. The audit refuses the same shape
            # for the same reason: consuming an option both misnames the value and silently
            # switches off the flag it swallowed, and here it would send the result somewhere the
            # caller is not reading.
            if arg.startswith("-"):
                complain("--out needs a path, got the option [%s]" % arg)
                return 2
            out = arg
            expecting_out = False
        elif arg == "--nul":
            nul = True
        elif arg == "--json":
            nul = False
        elif arg == "--out":
            expecting_out = True
        elif arg in ("-h", "--help"):
            complain(USAGE)
            return 0
        else:
            rest.append(arg)
    if expecting_out or not rest or rest[0] not in SUBCOMMANDS:
        complain(USAGE)
        return 2
    subcommand = rest[0]
    try:
        result = SUBCOMMANDS[subcommand][0](rest[1:])
        payload = rendered(subcommand, result, nul)
    except Unparsed as exc:
        complain("%s: %s" % (subcommand, exc))
        return 1
    except OSError as exc:
        complain("%s: %s" % (subcommand, exc))
        return 1
    try:
        write_result(payload, out)
    except (OSError, ValueError) as exc:
        complain("%s: writing the result failed: %s" % (subcommand, exc))
        return 1
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv[1:]))
    except SystemExit:
        raise
    except BaseException as exc:  # any failure at all is a failed parse, never a result
        complain("%s: %s" % (type(exc).__name__, exc))
        sys.exit(1)
