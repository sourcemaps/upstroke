---
id: PR441-ALLOWLIST-RESET-CREDITS-CALL-COUNT
severity: P3
disposition: deferred
category: docs-contract
pr: 441
reviewed_sha: dc7af793fb624aa58d69b29ea3890a083a05b943
location: effects/allowlist.toml:830
provenance: fix_regression
first_bad: 24f214d7
guard: the next change to `effects/allowlist.toml`, which re-measures that count
---

# The tracker's effect allowance miscounts its `reset_credits` test calls

Filed deferred from #441's review of the reviewed commit. Audit evidence only: the count is prose in an instrument's review text.

## Failure sequence

    the `src/tracker.rs` entry's review text records three tracker test calls to `reset_credits`
    -> the reviewed commit has four, at lines 594, 631, 654 and 1044 of `src/tracker.rs`
    -> the new reader fixture uses the allowance without updating its stated count

The sentence, at `effects/allowlist.toml:830` to `:831`:

```text
The reset reader adds no production call of a denied primitive; its three \
test call sites of `reset_credits` use this existing allowance. \
```

Counted again at the head this row is filed on, after this pull request's later commits, `src/tracker.rs` has five: lines 594, 631 and 654, line 881 in `the_reset_bearer_reaches_no_child_from_the_ambient_environment`, and line 1283 in the helper `reset_report`.

## What the change that takes this up should do

Re-measure the count against the tree it lands on and state it. No code change. `effects/allowlist.toml` is one of the effect allowlists `PROCESS.md` §8 names, so the change that carries it is the owner's to merge.
