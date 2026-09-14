---
id: PR286-R4-ESCAPED-FENCE-RUNS-COST-QUADRATIC-PARSE-TIME
severity: P2
disposition: deferred
category: performance
pr: 286
reviewed_sha: d2bf5ae8fcf6885daec83b275f088a80a7416838
location: scripts/pr-review-parse.py:482
provenance: fix_regression   # found by #286's round-4 regression lens, review-286-regression.log
first_bad: 553cfddf2cee8d36115414372e0d0763be3d9cad   # the per-run read of the rest of the line starts here; fe7bc045 (round 4) made it eight to ten times dearer
guard: no gate row parses a review with many fence runs on one line, so nothing bounds the cost -- the missing outcome is the 65,237-character witness below reading READY with one merge call from a reader whose work grows linearly with the line
---

## Failure sequence

**The per-document measurement is the finding. Whole-gate timing does not measure it**, and the last
part of this section says why, because it has already been used to discount this finding once.

The witness is the regression lens's own. It is a four-backtick `text` block whose one content line
is `~~~\!` written 13,000 times, followed by a clean review whose only verdict is a valid `PASS`. It
is 65,237 characters:

```python
'````text\n' + ('~~~' + chr(92) + '!') * 13000 + '\n````\n\n' + clean_review
```

`unresolved_material` reads every fence run inside a block the verdict is not read from
(`scripts/pr-review-parse.py:752`), and `CONTENT_FENCE_RUN` (`:336`) hands `names_json` the whole rest
of that run's line. At this head `names_json` asks `rendered_language`. At `:482` that runs
`INFO_ESCAPE.sub` over the entire suffix, calling the Python function `resolved` once for every
backslash escape or reference in it. The line holds 13,000 `~~~` runs, and after run *k* there are
13,001 - *k* escapes. That is n(n+1)/2 = 84,506,500 calls for n = 13,000. Wrapping `INFO_ESCAPE`
confirmed the count exactly at 1,625 and 3,250 runs: 1,321,125 and 5,282,875 calls. The answer is
right, and slow.

Executed 2026-09-14 at `6efbc2b1`, whose parser is byte-identical to `d2bf5ae8`'s. Each tree is a
`git archive` in a scratch directory: master is the merge base `2b504d4f`, unchanged at
`origin/master` `8b28944f`, and round 3 is `7bf177af`. The witness and the recording `gh` stub were
rebuilt from the lens's description and are byte-identical to the lens's own copies. Times are
wall-clock seconds; for every parser run the child's own CPU time agreed to within 0.01 s. The
one-minute load average was 11 to 14 on 32 cores. Every parser run exits `0` and returns `PASS`:

| the 65,237-character witness | master | round 3 | head |
|---|---|---|---|
| `scripts/pr-review-parse.py review`, three runs | 0.018, 0.018, 0.019 | 1.304, 1.297, 1.347 | **12.753, 12.613, 12.771** |
| `timeout 10 bash scripts/pr-ready-audit.sh --enqueue 999` | exit `0`, READY, 1 merge call, 0.085 | exit `0`, READY, 1 merge call, 1.444 | **exit `124` at 10.001, 0 merge calls; only the table header printed** |
| the same audit, no watchdog | not run | not run | exit `0`, READY, 1 merge call, 11.292 |

`markdown-it-py` 3.0.0 renders the same document in 0.002 s, as one `language-text` block and one
`language-json` block.

**What it costs.** The verdict does not change: every parse returns `PASS`, and with no watchdog the
head's audit enqueues. The cost is time. The document is review text, and each time the audit parses
this comment it spends 11 to 13 seconds of CPU on this box. A caller with a shorter deadline gets no
result. This is an executed slowdown, not evidence of infinite recursion.

**The cost grows with the square of the line.** The parser was run on the same document with fewer
runs, once per size:

| `~~~\!` runs (characters) | master | round 3 | head |
|---|---|---|---|
| 1,625 (8,362) | 0.019 | 0.042 | 0.256 |
| 3,250 (16,487) | 0.019 | 0.138 | 0.827 |
| 6,500 (32,737) | 0.019 | 0.416 | 3.201 |
| 13,000 (65,237), median of the three above | 0.018 | 1.304 | 12.753 |

Master is flat. At each doubling round 3 grows 3.0 to 3.3 times and the head 3.2 to 4.0 times.

**Round 4 did not create the shape; it multiplied it.** Here is every parser this pull request wrote,
once per size, in a separate run at a load average of 8:

| parser | 1,625 | 3,250 | 6,500 | 13,000 |
|---|---|---|---|---|
| `2b504d4f`, the merge base | 0.018 | 0.021 | 0.018 | 0.019 |
| `9f8ba822` | 0.021 | 0.019 | 0.020 | 0.023 |
| `553cfddf` | 0.025 | 0.042 | 0.112 | 0.380 |
| `4ac348df` | 0.034 | 0.065 | 0.190 | 0.710 |
| `e8816ad2`, round 3's parser | 0.043 | 0.109 | 0.371 | 1.430 |
| `fe7bc045`, round 4's parser = head | 0.217 | 0.794 | 3.036 | 12.039 |

`553cfddf` asked the content rule of every fence run wherever it stands on its line, and read the
rest of that line after each one. Before it, the pattern was anchored at the start of a line. Each
later fix added work to that read. Round 4's version calls a Python function per escape, and on this
witness it is eight to ten times dearer than round 3's.

**Whole-gate timing does not measure this, and must not be used to weigh it.** A triage of this
finding published at 14:18Z, and since withdrawn, discounted it with a complete-gate comparison from
#286's first conformance draw (`review-286-conformance-121215.log`). That draw timed master at
`18.007 s` and `d2bf5ae8` at `24.369 s`, both exiting `0` and both exceeding a ten-second watchdog.
The triage read that as the PR making an existing slowness about 35 % worse, not introducing it. That
compares different work:

- a complete-gate time is the sum of every fixture the gate runs;
- the two gates are different files: the head's `.github/scripts/test-pr-ready-audit.sh` has 3,020
  lines against master's 2,335;
- neither gate holds a document of this shape. Their only `~~~` lines open or close single
  tilde-fenced blocks.

Re-measured here, from `git archive` trees, the complete gate took 22.24 s on master and 32.66 s at
the head, both exiting `0` with `test-pr-ready-audit: ok`: a ratio of about 1.5. On the witness, the
parser's ratio is about 700 (0.018 s against 12.753 s). Reading the whole-gate ratio as this finding's
cost understates it by more than two orders of magnitude.

## What the change that takes this up should do

Bound the work per content fence run. `names_json` only asks whether the first word of the resolved
info string is `json`, folded. `INFO_ESCAPE` makes one left-to-right pass, and each replacement
depends only on the text it matched. So a reader can stop as soon as the resolved text can no longer
begin a first word equal to `json`; on this witness that is the first `!`. **Stopping at the end of
the first word is not enough**: this line holds no whitespace, so its first word is the whole rest of
the line, and that reader is still quadratic. Whatever the change, `names_json` must still agree with
`markdown-it-py` 3.0.0 on round 4's measured corpus.

Then add this witness to the gate: READY with one merge call, and a bound that fails a quadratic
reader. A count of resolver calls holds however fast the machine is; a wall-clock bound does not.
