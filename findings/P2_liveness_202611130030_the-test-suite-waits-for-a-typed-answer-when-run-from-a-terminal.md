---
id: TEST-SUITE-PROMPTS-ON-A-TERMINAL
severity: P2
disposition: deferred
category: liveness
pr: TBD
reviewed_sha: 2b16cec2b9a4cd6f018bb0a285071a9e8946bcf9
location: src/interaction.rs:451
provenance: pre_existing
first_bad:
guard: the change that keeps every engine test from choosing the terminal answer source, by giving the tests an interaction mode or answer source that never reads the process's standard input, with a witness that runs a test which parks a question under a pseudo-terminal and requires it to finish
---

# The test suite waits for a typed answer when an engine test parks a question and standard input is a terminal

## Failure sequence

`cargo test` is started with a terminal as its standard input (from a shell, or from a `tmux` pane), and an
engine test parks a question:

    `cargo test` passes its own standard input to the test binary
    -> a_zero_gate_schema_4_resume_probes_its_recorded_shell_when_todays_config_names_an_unavailable_one first
       makes a fresh schema-4 run, `run_with(&topology_options(&repo), …)` (src/engine/tests.rs:21588);
       `topology_options` (src/engine/tests.rs:17782) is built on `options()` (src/engine/tests.rs:781), which
       leaves `interaction` unset
    -> `prepare_fresh` takes `opts.interaction.unwrap_or(analysis.config.interaction_mode)`
       (src/engine/topology/entry.rs:223); `temp_engine_repo` writes no `upstroke.toml`, so the mode is the
       default, `OnBlock`, which is interactive
    -> `drive` calls `answers_for` (src/interaction.rs:442) at src/engine/topology/entry.rs:872; the process's
       standard input is a terminal, so it returns `TerminalAnswers` (src/interaction.rs:451)
    -> the stub parks a question on t1 and t2 depends on t1, so `select` returns `HardBlock`
       (src/engine/topology/select.rs:388) -> `hard_block` resolves it (src/engine/topology/run.rs:565) ->
       `DeliveredFirst::resolve` finds no delivered answer and asks the attended source
       (src/engine/topology/entry.rs:989) -> `TerminalAnswers::resolve` (src/interaction.rs:304) prints the
       question and blocks in `read_line` on the test process's own standard input until a line or end of input
       arrives
    -> nobody types, and the library's tests stop; the test's resume, `resume_options`
       (src/engine/tests.rs:21604), is never reached

The legacy schema 1-3 paths' calls to `answers_for`, `src/engine/resume.rs` (:736) and
`src/engine/coordinator.rs` (:242), are not what this test reaches: each of those paths opens its log with
`EventSite::LegacyOpenLog` (resume.rs:743, coordinator.rs:248), and `resume_harness` sends a schema-4 resume to
`engine::topology::entry` instead (src/engine/resume.rs:50-57).

**Observed** on the build machine on 2026-11-12 and 2026-11-13, in two baselines of pull request #376 at
`1ea3aa31` started from a `tmux` pane. Each time `cargo test --all-targets --all-features` reported 731 of 3961
library tests and then nothing for over ten minutes (the first was left 25 minutes), until the operator ended it.
The last line before the silence was:

    test engine::tests::a_zero_gate_schema_4_resume_probes_its_recorded_shell_when_todays_config_names_an_unavailable_one has been running for over 60 seconds

In the stopped process 50 `engine::tests` threads were waiting on a lock and one was blocked reading. The same
suite at the same `src/` passed when started without a terminal.

**Who sees it.** CI does not: its jobs' standard input is not a terminal. A maintainer who runs `cargo test` from
a shell does, and so does any baseline started from a terminal. Until it is repaired, the operator runs baselines
with standard input from `/dev/null`.

## Reproduction

At `master` `2b16cec2`, with the library's test binary built:

    <test binary> --exact engine::tests::a_zero_gate_schema_4_resume_probes_its_recorded_shell_when_todays_config_names_an_unavailable_one

- With standard input `/dev/null` it passes in 0.44s.
- Under a pseudo-terminal, `script -qec "<command> --nocapture" /dev/null`, it prints the question
  `` question q-… [clarify]: Task `t1` — Implement the widget — parking t1; the run continues ``, its options,
  and `` answer (a number picks an option, `skip` fails this task, empty leaves it parked): ``, and then waits
  until it is killed (the operator's `timeout 30` ended it).

## What the change that takes this up should do

Make the engine tests never choose the terminal answer source: set, in both test helpers — `options()`
(src/engine/tests.rs:781, which `topology_options` builds on) for runs, and `resume_options`
(src/engine/tests.rs:832) for resumes — an interaction mode or an answer source that never reads standard
input, rather than changing what a user's interactive run does. Setting only `resume_options` leaves the cited
test hanging in its fresh run. Add a witness that runs a question-parking test under a pseudo-terminal and
requires it to finish: the cited test's fresh run, `run_with(&topology_options(&repo), …)`, must finish (and
park) under a pseudo-terminal. The change must not weaken what `TerminalAnswers` does for a real
interactive run.
