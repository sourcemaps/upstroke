<h1><img src="assets/upstroke-wordmark.svg" alt="upstroke" height="80"></h1>

[![CI](https://github.com/sourcemaps/upstroke/actions/workflows/ci.yml/badge.svg)](https://github.com/sourcemaps/upstroke/actions/workflows/ci.yml)
[![license](https://img.shields.io/github/license/sourcemaps/upstroke.svg)](LICENSE)

`upstroke` runs a written plan through the coding-agent CLIs you already use (Claude Code, Codex,
GitHub Copilot), unattended, and commits a task's work only after it passes the run's gates (your
build and test commands) and, by default, a model review.

A coding agent will tell you a task is done. Whether it is, you find out yourself: run the tests,
read the diff, send it back with the failure, try a stronger model, then again for the next task.
In [a recorded run](https://upstroke.rs/) an agent's fix built cleanly, passed all 722 tests and
was still wrong; the reviewer rejected it and the retry got it right. `upstroke` does that
supervision for you.

It is for work you have already designed and written down as a plan. You come back to:

- **Verified work on its own branch**, `upstroke/run-<id>`; your own branch is not touched.
- **Retries and escalation, done for you.** A failed gate or rejected review sends the task back
  with the gate log or the reviewer's required changes, then up to a stronger model, and when the
  chain runs out it becomes a question for you. An agent that needs a decision, or a change that
  cannot be reviewed, asks sooner. A question holds that task and its dependents while the rest
  keeps going.
- **Cheaper models first.** By default fixes, tests, docs and chores start on a small model,
  implementation on a mid-tier one, design and review on a frontier one.
- **A full account.** A ledger of every attempt, model and reported cost, and an event log that
  resumes the run after a crash, a question or a budget stop.

## How it works
Each task in the plan ([Plans](#plans)) runs in its own git worktree; a run an older binary created (schema 1–3) resumes on the sequential path instead. Gates are your `[[gates]]`
commands or, with none configured, ones derived from `Cargo.toml`, `go.mod` or a `package.json`
test script; any other repository has no gates until you add them. A change verified against an
older head is checked again on the combined tree before it lands.

## Status
`v0.1.0`, the sequential conductor, is the only tagged release ([CHANGELOG](CHANGELOG.md)). This
README describes `master`, where v0.2 is in progress and unreleased; capacity is tracked for the
whole machine and reported, not acted on.

## Install
From a checkout of `master`, `cargo install --locked --path .` builds `upstroke` into Cargo's `bin`
directory (`~/.cargo/bin` unless you set another root), which must be on your `PATH`. You need Rust 1.85+ (edition 2024), Git 2.41+ (2.40 is refused) and
[Claude Code](https://docs.claude.com/en/docs/claude-code/overview) on `PATH`, on Windows, macOS or
Linux. A default run also needs the Codex CLI: its review plan names an alternative reviewer from
another model family, which is probed before the run is created, unless
`[routing] review = { cross_family = false }` ([design/17](design/17_design_configuration_reference.md), "Which CLIs a schema-4 run needs").
On Windows, Codex can review but not implement.

## Quick start
| Command | What it does |
|---|---|
| `upstroke connect` | Discovers agent CLIs and writes `~/.upstroke/pools.toml`. |
| `upstroke validate plan.md` | Prints the task graph, model chains, gates and reviewers, at zero spend. |
| `upstroke run plan.md --dry-run` | The same preview; no agent is started. |
| `upstroke capacity` | Checks sign-in once per agent and shows each pool's estimated remaining, from every run on this machine. |
| `upstroke capacity --refresh` | First asks each pool's billing endpoint what it has drawn this month, for pools you opted in. |
| `upstroke run plan.md --budget 15` | Runs for real on a `upstroke/run-<id>` branch; needs a clean working tree. |
| `upstroke status [<run-id>]` | The ledger: attempts, models, cost. |
| `upstroke answer <question-id>` | Answers a question that parked a task. |
| `upstroke decide <run-id> [<question-id>]` | The run's reviewer proposes an answer to a parked question, within `--max-cost-usd` (default $5 a run); it answers nothing. |
| `upstroke resume <run-id> [--budget 30]` | Continues a stopped run from its event log. |
| `upstroke export-decisions <run-id> [--format csv]` | Exports a finished run's routing decisions. |

`run` and `resume` exit `0` when the run is complete, `2` when it is parked on unanswered
questions, `3` when it stopped at its budget (raise it and `resume`), and `1` when it halted or
the command failed. A command-line usage error also exits `2`. `decide` exits `1` when any reply
or question was refused or it stopped with questions left, after its report; it decides a run
once, and runs only for a host run whose reviewer is Claude Code
([design/18](design/18_design_cli_surface.md)).

A run has no budget unless you set `--budget` or `[budgets]`. A budget stops new work once the
reported spend of finished work reaches it; work already started, and its reviews, can exceed it.
Costs are API-equivalent, so notional on a subscription, and a total marked `?` is missing a price
a CLI did not report.

## Plans
Tasks are top-level `##`/`###` sections, excluding acceptance headings; a plan with no task
sections uses its top-level checklist items or numbered steps instead. Dependencies follow document
order unless annotated: an HTML-comment annotation grammar refines a plan without changing how it
renders ([design/09](design/09_design_plan_ingestion.md)).
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
budgets are repo-level, in `upstroke.toml`. Any of those settings can also be written once for the
whole machine, in `~/.upstroke/upstroke.toml`: the repository's file is laid over it and wins
wherever both speak. Zero config works; the configuration reference is
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
- **Agents edit files; the engine owns git and never pushes.**
- **The engine never calls a model API.** Models run only inside the vendors' own CLIs.
- **A run makes no network request of its own.** One command does, and only when asked:
  `upstroke capacity --refresh` reads the documented billing endpoint of each pool you opted in,
  through `curl`, with a key from an environment variable you name
  ([design/13](design/13_design_capacity_engine.md)).
- **Ground truth is the diff, not the transcript.** Gates and reviewers get the diff the engine
  captured.
- **An empty diff can never pass.**
- **Narrow permissions; reviewers are read-only.** Claude Code and Copilot workers get file tools
  plus exactly your gate commands. On the host runner a Codex worker runs in Codex's
  workspace-write sandbox, which limits what it writes, not which commands it runs. No worker gets
  a skip-all-permissions flag; the design's one exception is Codex under the container runner,
  where the container is the boundary ([design/20](design/20_design_safety_and_permissions.md)).
- **Review is independent unless you configure otherwise** ([design/11](design/11_design_verification_ladder.md)).
  A task that ran on the reviewer's own CLI and model is reviewed by a model from another family;
  the same model pinned through a different CLI is not detected. A fresh run is not created if
  that alternative cannot be probed ([design/17](design/17_design_configuration_reference.md));
  only a run an older binary created (schema 1–3) warns and falls back to the
  frozen same-model reviewer. With `[routing] review = { cross_family = false }` a model can
  review its own work, and the run warns, naming the tasks.
  A configured blast-radius second opinion is stricter: it needs both families, and both must pass.
- **Host commands are cleaned up.** On Windows every supervised command is assigned to a
  kill-on-close Job Object before its primary thread runs, so its ordinary descendants are
  terminated after a direct-child exit, a timeout or abrupt conductor death; Unix uses a process
  group and a cleanup reaper. Containers differ: on Windows a killed conductor's containers last
  until the next `run` or `resume`.

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

Outside contributions are not being accepted at present; [CONTRIBUTING.md](CONTRIBUTING.md) says
why, and carries the workflow and the Contributor Licence Agreement. Bug reports and questions are
welcome as issues. Rust changes follow the [coding standards](CODING_STANDARDS.md), and the design
lives in [DESIGN.md](DESIGN.md).
