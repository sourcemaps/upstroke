<h1><img src="assets/upstroke-wordmark.svg" alt="upstroke" height="80"></h1>

[![CI](https://github.com/sourcemaps/upstroke/actions/workflows/ci.yml/badge.svg)](https://github.com/sourcemaps/upstroke/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/upstroke.svg)](https://crates.io/crates/upstroke)
[![license](https://img.shields.io/github/license/sourcemaps/upstroke.svg)](LICENSE)

A headless orchestration engine for AI coding agents. You and a frontier model design a piece of
work; `upstroke` runs it unattended: the plan becomes a dependency graph of typed tasks, each task
goes to an existing coding-agent CLI with a model chosen per task, gates and a review check every
result (independent unless configured otherwise; see [Guarantees](#guarantees)), and only what
passes is committed. It never edits a file, never runs an agentic loop and never calls a model
API. It is the conductor, not an instrument.

## Status
`0.1.0` is released on crates.io and works end to end. v0.2 (worktree-per-task parallelism, a merge
queue, capacity-driven routing, more adapters) is in progress on `master` and unreleased. [See a run, start to finish →](https://upstroke.rs/)

## Install
`cargo install upstroke` installs the released `0.1.0`; `cargo build --release` in a checkout builds
`master`. You need Rust 1.85+ (edition 2024), Git 2.41+ (2.40 is refused) and
[Claude Code](https://docs.claude.com/en/docs/claude-code/overview) on `PATH`, on Windows, macOS or
Linux. A default run on `master` also needs the Codex CLI: its review plan names an alternative
reviewer from another model family, which the run probes before it commits anything, unless
`[routing] review = { cross_family = false }` ([design/17](design/17_design_configuration_reference.md), "Which CLIs a schema-4 run needs").
On Windows every supervised command is assigned to a kill-on-close Job Object before its primary
thread runs, so its ordinary descendants are terminated after a direct-child exit, a timeout or
abrupt conductor death.

## Quick start
| Command | What it does |
|---|---|
| `upstroke connect` | Discovers agent CLIs and writes `~/.upstroke/pools.toml`. |
| `upstroke validate plan.md` | Prints the task graph and sourced model chains, at zero spend. |
| `upstroke run plan.md --dry-run` | Everything except the agents. |
| `upstroke capacity` | Checks sign-in once per agent and shows each pool's estimated remaining. |
| `upstroke run plan.md --budget 15` | Runs for real, committing per task on a `upstroke/run-<id>` branch. |
| `upstroke status [<run-id>]` | The ledger: attempts, models, cost. |
| `upstroke answer <question-id>` | Answers a question that parked a task. |
| `upstroke resume <run-id> [--budget 30]` | Continues a stopped run from its event log. |
| `upstroke export-decisions <run-id> [--format csv]` | Exports a finished run's routing decisions. |

Exit codes: `0` complete · `1` a task failed and the run halted · `2` tasks parked on unanswered
questions · `3` stopped at its budget (raise it and `resume`).

## Plans
Any markdown works; an HTML-comment annotation grammar refines it without changing how it renders
([design/09](design/09_design_plan_ingestion.md)).
```markdown
## Design the pagination API
<!-- upstroke: id=api-design kind=design depends= tier=frontier out=api-contract -->

## Fix off-by-one in list endpoint
<!-- upstroke: id=fix-obo kind=fix depends=api-design min=mid paths=src/api/** -->
```
A task is ready to run unattended when its worker and reviewer can reach the same verdict:
observable acceptance criteria, named boundary conditions, inspectable evidence, choices settled
while you are present.

## Configuration
Pools are user-level, in `~/.upstroke/pools.toml`, written by `upstroke connect`; routing, gates and
budgets are repo-level, in `upstroke.toml`. Zero config works; the configuration reference is
[design/17](design/17_design_configuration_reference.md).
```toml
[routing]
fix = { chain = ["small", "mid", "frontier"], attempts_per = 2 }
[budgets]
run_usd = 15.0                         # api-equivalent; reaching it ends the run (exit 3)
[[gates]]
name = "test"
cmd = "cargo test"
```

## Guarantees
- **Agents edit files; the engine owns git.**
- **The engine never calls a model API.**
- **Ground truth is the diff, not the transcript.**
- **Narrow permissions; reviewers are read-only.**
- **Independent review, with an explicit fallback** ([design/11](design/11_design_verification_ladder.md)).
  Where a task runs on the model that would review it, the review is rebound to another model
  family. On `master` a fresh run (schema 4) probes every reviewer its plan names, and is not
  created if the alternative cannot be probed ([design/17](design/17_design_configuration_reference.md));
  a run an older binary created (schema 1–3) warns and falls back to the
  frozen same-model reviewer rather than claiming independence. With
  `[routing] review = { cross_family = false }`, or where no other family is configured, no
  alternative is named and the same model can review its own work. A configured blast-radius
  second opinion is stricter: it needs both families, and both must pass.
- **An empty diff can never pass.**
- **Estimates are never optimistic.**

## Security
Host-run mode is for trusted repositories and plans, because gates run the candidate's own build
and test code as your OS account. Use a dedicated OS account or VM for untrusted input
([design/20](design/20_design_safety_and_permissions.md)).

## Documentation
- [DESIGN.md](DESIGN.md) and [`design/`](design/): the design, one section per file.
- [MAINTAINING.md](MAINTAINING.md): how changes are reviewed and merged.
- [CONTRIBUTING.md](CONTRIBUTING.md), [CODING_STANDARDS.md](CODING_STANDARDS.md) and [CHANGELOG.md](CHANGELOG.md).
- [`docs/internals/`](docs/internals/): per-module notes; [`findings/`](findings/): the review finding ledger.

## Licence
**Apache License 2.0**, see [LICENSE](LICENSE) and [NOTICE](NOTICE). The name is not part of the
grant (Apache-2.0 §6): use "upstroke" to refer to this project, not to present a modified version
as the original.

Contributions are welcome under the CLA and workflow in [CONTRIBUTING.md](CONTRIBUTING.md); Rust
changes follow the [coding standards](CODING_STANDARDS.md), and the design lives in
[DESIGN.md](DESIGN.md).
