---
id: CAP-CROSS-FAMILY-REVIEWER-CANNOT-BE-DROPPED
severity: P2
disposition: deferred
category: liveness
pr: TBD
reviewed_sha: 2b16cec2b9a4cd6f018bb0a285071a9e8946bcf9
location: src/review.rs:268
provenance: pre_existing
first_bad:
guard: the change that lets a run go on when the cross-family reviewer's provider is out of capacity, by a configuration that drops the alternative or by recognising the provider's capacity refusal and falling back to same-model review as preflight already does for a failed probe, with a witness of a run whose alternative reviewer is refused for capacity
---

# A run cannot drop a cross-family reviewer whose provider is out of capacity

## Failure sequence

`plan_for` in `src/review.rs` (line 268) builds a run's review plan. Its primary reviewer is the review tier's
`[[pins]]` entry; where the primary's model family is in the catalog, it also names an alternative from another family
at that tier, used for the tasks the primary's own model implemented. With the self-hosted runs' configuration (worker
and reviewer both pinned to `claude-code/claude-opus-5-5` at the `frontier` tier), the alternative is
`codex/gpt-6-astra`, so every task's review is by Codex.

    the operator pins the worker and the reviewer to `claude-code/claude-opus-5-5` at the `frontier` tier
    -> `plan_for` names `codex/gpt-6-astra` as the alternative, and every task's review is by Codex
    -> pinning around it is refused: pins must name a catalog model, and a pin of the Claude Code CLI's `opus`
       alias was refused ("pin references unknown model `opus` for agent `claude-code`")
    -> if the Codex CLI's probe fails, a fresh run's creation stops at its step P3c ("creating run … stopped at
       P3c: agent error: `codex` --version exited with Some(1): …"); observed by the operator on 2026-11-12 with a
       stand-in `codex` that exits 1
    -> if the Codex CLI is installed and its account is out of capacity, the probe passes and the reviews fail at
       run time instead: on 2026-11-12 the account's weekly window was at 100%, and passes failed with "Selected
       model is at capacity" and "exceeded retry limit, last status: 429 Too Many Requests"

`src/engine/preflight.rs` (near line 249) has a path that drops the alternative and falls back to same-model review
when its probe fails, yet the refusal at P3c above was still observed. Neither case leaves the operator a way to run
with Claude reviews alone: the only way the operator found to run without Codex was to turn per-task review off
(`[routing] review = { enabled = false }`) and review each pull request outside the run.

The owner's direction for that case, 2026-11-12: "If codex runs out, set upstroke to just use claude opus reviews
running on max". The engine offers no way to do that.

## What the change that takes this up should do

Let a configuration drop the alternative reviewer; or recognise a capacity refusal from the alternative and fall back
to same-model review with a warning, as preflight does for a failed probe. Add a witness of a run whose alternative
reviewer is refused for capacity. This belongs with the capacity work (`design/13_design_capacity_engine.md`).
