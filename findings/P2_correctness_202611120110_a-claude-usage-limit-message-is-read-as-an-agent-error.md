---
id: CAP-CLAUDE-USAGE-LIMIT-READ-AS-AGENT-ERROR
severity: P2
disposition: deferred
category: correctness
pr: TBD
reviewed_sha: 487f733af215ae92f20bdeb71aee356840b1fe42
location: src/agent/adapter.rs:203
provenance: pre_existing
first_bad:
guard: the change that makes the rate-limit vocabulary recognise the message Claude Code prints when an account reaches its limit, with a parser test on the verbatim message
---

# A Claude usage-limit message is read as an agent error and spends the task's attempts

## Failure sequence

`looks_rate_limited` in `src/agent/adapter.rs` (line 203) is the one vocabulary the Claude, Codex
and Copilot adapters use to decide whether a failed attempt was a rate limit. Its phrases are
"usage limit", "rate limit", "rate_limit", "limit reached", "limit exceeded", "overloaded",
"quota exceeded", "insufficient credits", "out of credits", "premium request", "monthly limit" and
"429". Its notes, `docs/internals/agent/adapter.md`, say the phrases cover the wording Claude Code
prints, and give "5-hour limit reached" and "Weekly limit reached".

Claude Code now words it differently. In Upstroke run `01M740JX5N8HB94RNC7W2MM777`, on 2026-11-11:

1. The implementing account reaches its five-hour limit.
2. The CLI exits 1 with that message. The attempt's recorded failure is, verbatim:

   ```
   agent error (exit Some(1)): You've hit your 5-hour limit · resets 5:30am (UTC)
   ```

   with the detail `You've hit your 5-hour limit · resets 5:30am (UTC)`.
3. The vocabulary does not match: none of its twelve phrases occurs in that text.
4. The attempt is an agent error. `parse_output` in `src/agent/claude.rs` (near line 354) gives it
   the status `AgentError`, not `RateLimited`. The difference matters downstream: a `RateLimited`
   outcome is an outage, which does not spend the task's attempt allowance and is deferred and
   tried again (`src/ladder.rs`); an `AgentError` spends an attempt.
5. The retry meets the same limit seconds later and fails the same way.
6. Both attempts of the task's only rung are spent, and the task is parked on an unblock question.
7. Every other task does the same.

What it cost in that run: twenty-one attempts failed this way, each within seconds, across twelve
tasks. Each task used both attempts of its only rung and was parked on an unblock question, and the
run ended with no commit. The run's event log records all twenty-one with the same text.

The message carries the reset time ("resets 5:30am (UTC)"). Nothing reads it; that is not part of
this finding.

## What the change that takes this up should do

- **Add the current wording to the one vocabulary**, `looks_rate_limited`, so the message above is
  read as a rate limit. A successful attempt that merely mentions limits must still not count: the
  vocabulary is consulted only for a failed attempt, and that stays.
- **A parser test on the verbatim message**: a failed attempt whose output is
  `You've hit your 5-hour limit · resets 5:30am (UTC)` is `RateLimited`, not `AgentError`.
- **Not this change:** waiting until the stated reset, and switching accounts, are later steps of
  the capacity work.
