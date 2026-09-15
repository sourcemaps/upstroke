---
id: PR286-R4-A-JSON-TAGGED-EXAMPLE-QUOTED-IN-A-TEXT-BLOCK-IS-REFUSED
severity: P2
disposition: deferred
category: correctness
pr: 286
reviewed_sha: d2bf5ae8fcf6885daec83b275f088a80a7416838
location: scripts/pr-review-parse.py:752
provenance: fix_regression   # found by #286's round-4 regression lens, review-286-regression.log
first_bad: 9f8ba82218c89a1a9db102a2aed4fa199b8efcfa   # the plain tag; 4ac348df for short references; fe7bc045 (round 4) for eight-digit ones
guard: no gate row decides a json-tagged fence quoted inside a longer text fence beside one valid PASS -- neither a required-green row (READY, one merge call) nor a required-red row that pins the refusal as intended
---

## Failure sequence

**Whether this is a defect or intended conservatism is unresolved.** This file records what was
measured and the case on each side, and does not decide between them. The plain `json` spelling of
this example has been refused since this pull request's first fix commit, and that cost is disclosed.
Round 4 did not start the refusal. It moved eight-digit references, decimal and hexadecimal, from read
to refused.

The witness is the regression lens's own. It is a review whose only verdict is a valid `PASS`, with a
Markdown syntax example before it: a four-backtick `text` block quoting a fence tagged
`jso&#00000110;`.

`````text
Reviewed head: 98e7ecaafb1159d11304852ab8b220547bf691d2

Markdown syntax example:

````text
```jso&#00000110;
{"hello":"world"}
```
````

```json
{"reviewed_sha": "98e7ecaafb1159d11304852ab8b220547bf691d2", "base_sha": "11de280efd66c5a99b222cbc6a42a3f15955e6fe", "verdict": "PASS", "findings": []}
```
`````

`markdown-it-py` 3.0.0 renders two blocks. The first is `language-text`, and its content keeps
`` ```jso&#00000110; `` exactly as written. The second is `language-json`, the verdict. cmark-gfm was
not run: it is not installed on this box.

`unresolved_material` reads every fence run inside a block the verdict is not read from
(`scripts/pr-review-parse.py:752`), and hands the rest of that run's line to `names_json` (`:753`).
At this head `names_json` asks `rendered_language` (`:450`), which resolves `&#00000110;` to `n`. The
quoted example therefore names `json`, and the parse refuses with
`` the review carries material this parse could not account for as a block: [```jso&#000 inside the block at line 5] ``.

Executed 2026-09-14 at `6efbc2b1`, whose parser is byte-identical to `d2bf5ae8`'s. Each tree is a
`git archive` in a scratch directory. Master is the merge base `2b504d4f`, and its parser, audit and
`lane.sh` are unchanged at `origin/master` `8b28944f`. Round 3 is `7bf177af`. The witnesses and the
recording `gh` stub were rebuilt from the lens's description and are byte-identical to the lens's own
copies. Each cell is `<parser exit>/<audit state>/<gh pr merge calls>`, where the audit is
`scripts/pr-ready-audit.sh --enqueue 999`. The audit exits `0` in every cell:

| the quoted fence is tagged | master | round 3 | head |
|---|---|---|---|
| `jso&#00000110;` | `0/READY/1` | `0/READY/1` | **`1/NOT-READY/0`** |
| `&#x0000006A;son`, the hexadecimal twin | `0/READY/1` | `0/READY/1` | **`1/NOT-READY/0`** |
| `json`, the plain control | `0/READY/1` | **`1/NOT-READY/0`** | **`1/NOT-READY/0`** |
| no example: the clean review alone | `0/READY/1` | `0/READY/1` | `0/READY/1` |

Each refused audit reports `blockers=review-parse-failed:1,review-records-no-reviewed-sha,no-verdict`.
`markdown-it-py` 3.0.0 renders the plain control exactly as it renders the witness: one
`language-text` block holding the quoted fence, and one `language-json` block.

The same example was also run against every parser this pull request wrote, and the parser exit is
shown. Each spelling's refusal starts at one commit:

| the quoted fence is tagged | `2b504d4f` | `9f8ba822`, `553cfddf` | `4ac348df`, `e8816ad2` | `fe7bc045` (round 4) = head |
|---|---|---|---|---|
| `json`, `JSON` | `0` | `1` | `1` | `1` |
| `jso&#110;`, `&#x6A;son`, `jso&#x6e;`, `jso&#0000110;`, `&#x00006A;son` | `0` | `0` | `1` | `1` |
| `jso&#00000110;`, `&#x0000006A;son` | `0` | `0` | `0` | `1` |

So against round 3, what is new is the eight-digit reference only: round 4 widened the digit bound
from CommonMark's seven decimal and six hex digits to eight. Against master, every spelling is new.
`markdown-it-py` 3.0.0 renders a standalone fence with any tag in that table as `language-json`, or
`language-JSON`, which the parser folds.

**The case for intended conservatism.** The refusal is a stated cost. `553cfddf`'s message says:
*"A run CommonMark would not read as a fence now refuses when it names `json` inside a non-verdict
block."* The body's Risk section repeats it, as the direction every rule in the parser is wrong in.
Of round 2's widening, it says `names_json` can only find more names, which makes a candidate count
two or makes a block's content material. The gate's comment at
`.github/scripts/test-pr-ready-audit.sh:1833` considers quoting directly: *"A longer fence around a
shorter one is how a `text` block shows a fenced block, and only a fence run naming `json` is
material."* The reason is structural. The parser does not model raw-HTML blocks, so this example looks
the same to it as a `` ```text `` line hidden after `<!--` that swallows the real verdict. And
`jso&#00000110;` is `json` to the renderer round 4 transcribed, just as `jso&#110;` and `json` are. On
this reading, the head applies one rule to three spellings.

**The case for a defect.** The regression lens reported it as one: *"a correctly quoted example
containing no competing verdict now prevents acceptance. Equivalence of isolated info strings does not
establish equivalence of complete documents."* The renderer measured here shows the reader one
verdict, and it is `PASS`. Master enqueues this review, and so does round 3 in the eight-digit
spelling; the head sends it to a person. The disclosed cost names three shapes: a run after prose, a
run indented four spaces, and a run whose info string holds a backtick. It does not name an example
quoted inside a longer fence, and no gate row asserts that case in any spelling, as green or as red.

## What the change that takes this up should do

Decide the question once, for every spelling in the commit table above. All of them render
`language-json`, so a rule that reads some and refuses others is not the renderer's reading. Then pin
the answer in the gate. There are two choices:

- **Keep the refusal.** Add required-red rows for one tag from each row of that table. Each should
  assert `could not account for as a block` and zero merge calls, with a comment saying the refusal is
  deliberate and why.
- **Read quoted examples.** Add required-green rows for the same examples. Each should assert
  `enqueued #999` and one merge call, and every `MUT-BLOCK-CONTENT-NOT-MATERIAL` swallow case must stay
  red. This needs the parser to know when a fence line sits inside a raw-HTML block, which is the
  modelling its comments decline. It is a design change, not a one-line fix.
