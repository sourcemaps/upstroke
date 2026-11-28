## 3. Goals and non-goals

**Goals**

- Take a designed piece of work from plan to verified, committed code without supervision, asking only the questions that genuinely block progress.
- Route every task to the cheapest worker that can pass verification — where "cheapest" is measured against the user's real capacity pools, not list prices.
- Maximize the value of prepaid subscriptions: spend-down surplus capacity at the top tier, conserve when scarce, always leave a reserve for interactive use.
- Be agent-agnostic and vendor-neutral: official CLIs only, one adapter file per agent, cross-vendor review as a feature.
- Single static binary; first-class on Windows, macOS, Linux. Resumable, auditable, CI-embeddable.

**Non-goals**

- Not an agent: no file editing, no tool-use loop, no context management, no HTTP client, no model API keys. The binary links no HTTP client at all: the capacity tracker (§13), which reads billing and usage endpoints outside any run, starts `curl` for it, and the credentials it is lent are the billing-scoped ones an operator names (an organization admin key, a token with a plan-read permission), not keys for calling a model.
- Not a subscription proxy: never spoofs headers or re-exposes a subscription as an API endpoint. Subprocesses of official binaries only.
- Not a UI (the engine): panes, dashboards, and notifiers are thin clients of the event log. The design-pane product is v0.3, built *on* the engine.
- Does not repair bad plans. The design phase exists to make plans good; execution assumes they are. A person may still correct a running plan by amendment, which an agent may only propose (§5; decided 2026-11-25, not built).
- No cross-run learning in v0.x — but every routing decision is logged so a learned router is possible later.
