## 6. Architecture

```
            ┌────────────── Phase 1: Design (interactive) ─────────────┐
 user ◄────►│ frontier designer → tasks + conventions + decisions +    │
            │ per-task tier/min annotations                            │
            └───────────────────────────┬──────────────────────────────┘
                                        ▼ (plan frozen)
 plan.md ──► PlanAdapter ──► Plan IR (typed task DAG + artifacts)
                                   │
                                   ▼
                     Router (config ⊕ annotations ⊕ user override)
                     + CapacityEngine (pools, strategies, affinity)
                                   │
        ┌────────────── Scheduler ──┴──────────────────────────┐
        │  v0.1 sequential (skip-ahead past parked tasks)      │
        │  v0.2 tokio DAG + worktrees + merge queue            │
        │                                                      │
        │  Workspace ──► AgentAdapter ──► official CLI proc    │
        │      ▲               │                               │
        │      │               ▼                               │
        │    Gates ◄─── Outcome (diff, usage, session)         │
        │      │                                               │
        │      ▼                                               │
        │  Reviewer (read-only; optionally cross-vendor)       │
        │      │                                               │
        │      ▼ fail → retry (resume) → escalate rung → human │
        └──────┬───────────────────────────────────────────────┘
               ▼
      events.jsonl ──► resume · status · ledger · questions ──► notifiers (CLI / desktop / Telegram / Slack)
```

| Component | Responsibility |
|---|---|
| **PlanAdapter** | Parse a raw plan into the IR. One per format; `sniff()` for auto-detection. |
| **Router** | Resolve each task to an escalation chain of abstract tiers from three sources (config defaults, designer annotations, user override), applying blast-radius floors and `min` clips. |
| **Binder** | Resolve each tier to a concrete (agent × model × pool) at attempt time from everything connected — scoring capability fit (catalog), capacity headroom under the active strategy, and affinity; rebinds across pools on rate limits. Pins force fixed bindings. With a capacity handler (§13; decided 2026-11-25, not built), the pool is the handler's grant. |
| **CapacityEngine** | Track every quota pool (windows, credits, dollars, local), estimate remaining capacity, expose strategy decisions (conserve / value-max / spend-down) to the router and scheduler. Its evidence is machine-level and read from disk: the event log of every run on the machine, and the provider reports the capacity tracker stored (§13). |
| **Capacity tracker** | Outside the engine. `upstroke capacity --refresh` asks each pool's documented provider usage endpoint over HTTPS, through a `curl` subprocess, and stores the answer under the machine directory. `capacity --reset-credits claude` separately reads advisory voluntary reset grants from the admitted fixed endpoint. `capacity --subscription-usage claude` reads windows only through the signed-in CLI's control protocol; both data flags compose CLI windows and independent bearer grants. Explicit `--auth bearer` reads windows and grants from the same single fixed GET, even with both flags, and never starts the CLI. Both advisory readers need no pools, store nothing and consume no grant. The tracker dispatches no model work, owns no git and writes no event log, and `run` and `resume` never call it (§4.2). |
| **Scheduler** | Drain the DAG. Sequential in v0.1 — advancing past parked tasks to the next ready independent task. In v0.2 one coordinator owns events/admission while task pipelines run concurrently and one queue serializes integration. |
| **Workspace** | Git state: v0.1 run branch + per-task commits; v0.2 detached worktree-per-task, immutable candidate refs, staging worktree, and compare-and-swap integration. |
| **AgentAdapter** | Turn a `TaskRun` into a data-only `CommandSpec` for an official CLI and parse the outcome. One file per agent; it does not decide where the process runs. |
| **Runner** | Execute probes, workers, gates, and reviewers on the host or in a role-scoped container; owns cwd, mounts, environment, supervision, and timeout, never agent semantics or Git. |
| **Gates** | Configured shell commands (compile/test/lint) executed by the selected runner in the candidate workspace; failure logs become retry feedback. |
| **Reviewer** | Ordinary read-only worker profile emitting a structured verdict; optionally a different vendor from the implementer. |
| **Interaction** | Question/answer events, parking semantics, notifier plugins, CI degradation. |
| **Event log** | Append-only JSONL; source of truth for state, resume, status, questions, ledger, and the future decision-export dataset. |
