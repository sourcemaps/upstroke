---
id: POOL-ROUTES-ANY-CREDENTIAL-TO-ANY-PROVIDER
severity: P3
disposition: deferred
category: security-trust
pr: 441
reviewed_sha: d431d220f04a8fafd277b9bc55f4c113822167be
location: src/capacity.rs:250
provenance: pre_existing
first_bad:
guard: the change that refuses a usage_token_env naming a variable on a list the owner sets, when the pools file is read and again before a refresh resolves it
---

# A pool can send any credential in the environment to any usage provider

## Failure sequence

1. A pool in the operator's pools file names a usage provider and, in `usage_token_env`, the
   variable that holds its credential (`design/17_design_configuration_reference.md:30-33`).
2. The pools reader takes a name if it is syntactic, at most 64 characters, and does not begin with
   one of nine token prefixes (`src/capacity.rs:173-185`, `:250-255`, `:299-304`). So
   `CLAUDE_CODE_OAUTH_TOKEN`, or another provider's key variable, passes: read, not run.
3. `upstroke capacity --refresh` resolves that variable from the process environment
   (`src/main.rs:302`; `src/tracker.rs:171`) and sends its value, trimmed, in the request for
   whichever provider the pool names (`src/tracker.rs:171`, `:181-186`): one of four, at three hosts
   (`src/tracker/provider.rs:119-123`, `:137-189`). It sends nothing for a value that is empty once
   trimmed, or that still holds a control character, such as a line break, which is refused before
   `curl` starts (`src/tracker.rs:171-179`; `src/tracker/transport.rs:150-153`, `:180-190`, `:247`),
   nor for a pool of either GitHub provider that names no `usage_account`, which is skipped before
   the variable is read (`src/tracker.rs:165-170`; `src/capacity.rs:145-150`).
4. So the value of `CLAUDE_CODE_OAUTH_TOKEN`, or one provider's key, can be sent to another
   provider.

**How each step is known.**

- **Read:** each step, from the code at `d431d220`, not run.
- **Not run:** a refresh with such a pool.
- **Found by** the design note on the capacity reader's credential boundary of 2026-11-28
  (`~/selfhost-design-20261127/reader-credentials.md`, SHA-256
  `2e792ee4a70262a567c414c8769a5a481e89a41615dfdd7fe9f3aa9856d5cd56`), which ran nothing, and read
  again at `d431d220` for this record.

**What it needs:** the operator's own pools file. The design admits an endpoint only "with a
credential the provider issues for that purpose" (`design/20_design_safety_and_permissions.md:5`);
the pools reader checks a name's shape and nothing that ties it to a provider
(`src/capacity.rs:227-264`).

## Why P3 security-trust

Graded by ruling r114, at the grade the note proposed.

## What the change that takes this up should do

- Which names a pool may not use is the owner's decision.
- The refusal repeats no value and makes no transport call.
