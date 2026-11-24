## 19. Failure handling

| Failure | Detection | Handling |
|---|---|---|
| Agent binary missing / probe failure, or Claude Code not signed in | pre-flight | refuse to start |
| Agent spawn error | engine | halt run (environment, not task) |
| Agent output read fails, a reader panics, or the host refuses its thread | process funnel | settle the process tree and report the reader failure as an invocation error. Parent endpoints use nonblocking I/O and retry consecutive interruptions a bounded number of times. After the post-exit grace, release and join the reader before taking its capture. Classify a returned poll failure before cancellation; early supervisor exits retain secondary worker failures in the invocation error. Release does not mean EOF. The agent API retains bounded partial strings without a public completeness flag. The internal byte-preserving collector also reports the byte limit and observed EOF, for callers that require complete binary output |
| Agent stdin write fails, its feeder panics, or the host refuses the feeder thread | process funnel | settle the process tree and report the feeder failure as an invocation error. A broken pipe accepts the child's refusal of remaining input; other write failures, invalid write counts and excessive consecutive interruptions are errors. Feed stdin concurrently with output capture through an owned nonblocking endpoint. Collection and drop release, wake and join the feeder. Classify returned failures before cancellation and preserve secondary failures on early supervisor exits. After the post-exit grace, remaining input delivery is best-effort. Joining requires scheduling and finite local operations, without waiting for an escaped descendant to consume bytes or close its endpoint |
| Agent non-zero / timeout | adapter | attempt failure; feedback = stderr/transcript tail |
| Rate-limited | adapter signal | pool marked exhausted; a schema-4 worker's task runs again at once on another pool of the same agent not held exhausted (below); otherwise task deferred to reset or demoted per strategy (never below min) |
| Gate failure | gate runner | attempt failure; feedback = log tail |
| Review failure | verdict | attempt failure; feedback = required_changes |
| Chain exhausted | router | `Unblock` question to human (top rung); declined/CI → task Failed, dependents Blocked |
| Question parked, frontier non-empty | scheduler | continue independent tasks |
| Runnable frontier empty | scheduler | hard block (interactive) / end run reporting parked tasks (CI) |
| Budget or pool budget exceeded | ledger | stop scheduling; run ends `BudgetExceeded` |
| Merge conflict or code-attributed stale integration rejection (v0.2) | merge queue | publish nothing; atomically append rejection plus its replayable frozen Fix task, respecting hard pins/ceilings and the lineage-wide `max_merge_repairs`; infrastructure keeps its ordinary policy |
| Engine crash / power loss | — | `upstroke resume` replays the event log |

Pipe setup failures identify the affected stream and native operation. If
supervision fails and cleanup also fails, the original typed failure remains
primary and the additional cleanup failures are reported alongside it. A
successful cleanup does not change the original error category.

**Failing over to another pool (schema 4, as built).** When a worker's rate
limit would defer its task, and another pool of the same agent, in pools-file
order, is not held exhausted by the run's own signals, the attempt is settled
for a retry instead and the task runs again at once, in a fresh session and a
fresh worktree, with the same tier, agent, model and effort, on the first
such pool and under its `profile`. A failover spends no deferral and no
attempt. A pool stays held from its signal until an attempt it serves retires
the signal: one whose worker ran on it and ended in neither a rate limit of
its own nor an interruption, whatever the verdict on the code, or one with a
review pass on it that reached a verdict. So while another pool of the agent
is free, the run does not return to a refused pool on a reset it has only
read; with every pool held, a new attempt binds the first whose recorded
reset is two minutes past, or else the first in the file. A reviewer's limit,
and a run with one pool for the agent, defer as before, and a task that has
spent its deferrals asks its question as before. A run of schemas 1–3 does
not fail over.

**Waiting out a known reset (schema 4, as built).** When nothing is left to
run but deferred work, and a pool a deferred task waits on (the pool that
refused it for a reviewer's limit; every pool of the worker's agent for a
worker's limit, while all of them are held exhausted) has a recorded reset at
most 5 h 15 min ahead, the run waits until two minutes past the earliest such
reset, as one deferral, and prints once the pool, the reset and how long it
will wait. For a worker on an agent with more than one pool, the earliest
reset may already have passed: then the run waits only until two minutes past
it, and once the clock is two minutes past it there is no reset wait, only the
ordinary back-off. With one pool for the agent, or on a reviewer's limit, a
passed reset is not waited for, as before failover. A reset is spent once its
pool refuses again two minutes or more past it: the run no longer waits for it
or binds the pool on it, and holds the pool as one whose limit named no reset.
It sleeps in slices of at most ten minutes and chooses its next step again between them, so an answer that arrives meanwhile is taken first,
and it records one `defer_wait_elapsed`, for the whole wait, when the wait
ends. Otherwise the back-off is unchanged: 60 s, doubling to at most 600 s
while nothing progresses between waits. A dispatch counts as progress, so
when the limit refuses the same task after each wait, every wait is 60 s, and
after three deferrals its next refusal asks a question. A run of schemas 1–3,
resumed by the legacy coordinator, does not wait. A resume before the reset
starts the wait again, and the deferral it spends is counted again: every
resume wakes the deferred tasks, so the task meets the limit once more and is
deferred once more.
