---
id: PR258-INDEX-PACK-PACK-ONLY-IS-AN-UNSUPPORTED-UNIVERSAL
severity: P2
disposition: deferred
category: docs-contract
pr: 258
reviewed_sha: f6fd7fc0faa7e79618da922270ebe666f6100a5f
location: 48f2af7a:reviews/2026-09-09-g4-fix-temporary-object-fanout-record.md:75
provenance: fix_regression
first_bad: PR258-ROOT-WRITE-UNIVERSAL-OVERSTATES-PRODUCERS — the round-6 repair of that P2 is where the body's "writes only in `pack`" first appears: absent from the saved round-3 and round-5 bodies (17-r3-pr-body.md, 28-r5-pr-body.md), present twice in the round-6 and round-7 bodies (39-r6-pr-body.md, 46-r7-pr-body.md)
guard: the next edit of the #258 body takes it up — it rewords the Scope bullet for `src/workspace_manager.rs` and the guard cell of the `PR258-ROOT-WRITE-UNIVERSAL-OVERSTATES-PRODUCERS` row to what execution 33 opened and no more; no gate reads the body for this, and the rustdoc of `temporary_object_files` already says it at that scope
---

## Failure sequence

The pull request body says, in its Scope bullet for `src/workspace_manager.rs` and again in the
guard cell of the `PR258-ROOT-WRITE-UNIVERSAL-OVERSTATES-PRODUCERS` row, that round 6 names
`index-pack --stdin` at the same threshold "as one that writes only in `pack`". One execution
stands behind it:

    33-r6-strace-index-pack-vs-unpack-objects.log: git 2.43.0, one 200 000-byte object,
      core.bigFileThreshold=512, one invocation of `index-pack --stdin`
    -> that invocation opened pack/tmp_pack_*, pack/tmp_idx_* and pack/tmp_rev_* and nothing
       at the object root
    -> the body's "Round 6 executions" paragraph says exactly that, scoped to the execution
    -> the two sentences above say "writes only in pack" with no scope at all: a command-wide
       invariant over inputs, modes and git versions the execution did not cover

Reasoned, as the review says: other inputs, other modes and other versions remain unmeasured.
No execution contradicts the sentence, and none supports it. The record sentence at the location
above and the rustdoc of `temporary_object_files` (src/workspace_manager.rs:4542 at this sha) are
scoped to the execution and are not at fault; the body is not a tracked file, so the location
names the tracked sentence the body over-generalises. This is finding 1 of the `ultra` review of
`f6fd7fc0`, posted on the pull request as the SHA-bound comment, and the external universal that
review's lens grades P2 by rule.

## Since 2026-11-24

The record left master on 2026-11-24 (`bde77050`) and is read from history at the location above;
its own copies, the §1.1 sentence there among them, are archived evidence that nobody keeps true
and are not corrected. What stays open is the #258 body, whose two sentences still read as the
failure sequence quotes them in the body as saved on 2026-11-24
(`gh pr view 258 --repo sourcemaps/upstroke --json body --jq .body` as printed on 2026-11-24): line
15, the Scope bullet for `src/workspace_manager.rs`, "round 6 names `unpack-objects` as the
producer traced to the root and `index-pack --stdin` at the same threshold as one that writes only
in `pack`", and line 101, the guard cell of the `PR258-ROOT-WRITE-UNIVERSAL-OVERSTATES-PRODUCERS`
row, "index-pack --stdin at the same threshold as one that writes only in pack"; the body's "Round
6 executions" paragraph, line 36, still says it scoped to the execution.

## Why it is deferred

The finding is reasoned only: it carries no failing test, no reproduction and no mutation witness.
It bears on no Gate 4 pass-rule clause and on no row-10 evidence — the review says so of every
finding it returned — and the owner's rule for this pull request documents such findings and
merges. The correction is an edit to the body, which is not a commit.

## What the change that takes this up should do

Edit the #258 body: reword its two sentences — the Scope bullet for `src/workspace_manager.rs` and
the guard cell of the `PR258-ROOT-WRITE-UNIVERSAL-OVERSTATES-PRODUCERS` row — to what execution 33
showed, that invocation of `index-pack --stdin` opened only `pack` temporaries, the way the body's
own "Round 6 executions" paragraph and the rustdoc already say it; or execute the other inputs,
modes and versions and say in the body which. Do not narrow it into another universal: rounds 5
and 6 each did that once.
