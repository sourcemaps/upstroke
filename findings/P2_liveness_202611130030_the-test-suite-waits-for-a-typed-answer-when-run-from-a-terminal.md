---
id: TEST-SUITE-PROMPTS-ON-A-TERMINAL
severity: P2
disposition: deferred
category: liveness
pr: TBD
reviewed_sha: c060def5377c172905d5316b944a65b1d16abaeb
location: src/interaction.rs:451
provenance: pre_existing
first_bad:
guard: the change that keeps every engine test from choosing the terminal answer source, by giving the tests an interaction mode or answer source that never reads the process's standard input, with a witness that runs a test which parks a question under a pseudo-terminal and requires it to finish
---

# The test suite waits for a typed answer when its standard input is a terminal

## Failure sequence

`cargo test` is started with a terminal as its standard input (from a shell, or from a `tmux` pane):

    `cargo test` passes its own standard input to the test binary
    -> an engine test resumes a run through the helper `resume_options` (src/engine/tests.rs:832), which leaves
       `interaction` unset
    -> the run and resume paths take `opts.interaction.unwrap_or(analysis.config.interaction_mode)`
       (src/engine/preflight.rs:276, src/engine/topology/entry.rs:223 and :550), so the test resume runs in the
       configuration's mode
    -> `src/engine/resume.rs` (:736) and `src/engine/coordinator.rs` (:242) call `answers_for`
       (src/interaction.rs:442) with that mode
    -> the mode is interactive and the process's standard input is a terminal, so `answers_for` returns
       `TerminalAnswers` (:451)
    -> the test parks a question; `TerminalAnswers::resolve` (:304) prints it and blocks in `read_line` on the
       test process's own standard input until a line or end of input arrives
    -> nobody types, and the library's tests stop

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

At this branch's head, with the library's test binary built:

    <test binary> --exact engine::tests::a_zero_gate_schema_4_resume_probes_its_recorded_shell_when_todays_config_names_an_unavailable_one

- With standard input `/dev/null` it passes in 0.44s.
- Under a pseudo-terminal, `script -qec "<command> --nocapture" /dev/null`, it prints the question
  `` question q-… [clarify]: Task `t1` — Implement the widget — parking t1; the run continues ``, its options,
  and `` answer (a number picks an option, `skip` fails this task, empty leaves it parked): ``, and then waits
  until it is killed (the operator's `timeout 30` ended it).

## What the change that takes this up should do

Make the engine tests never choose the terminal answer source: set, in the test helpers — `resume_options` and
the run options' equivalent — an interaction mode or an answer source that never reads standard input, rather
than changing what a user's interactive run does. Add a witness that runs a question-parking test under a
pseudo-terminal and requires it to finish. The change must not weaken what `TerminalAnswers` does for a real
interactive run.
