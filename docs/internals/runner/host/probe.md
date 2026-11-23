# `src/runner/host/probe.rs`

Extended notes for [`src/runner/host/probe.rs`](../../../../src/runner/host/probe.rs).

The code is the authority for what it does; this file is the whole of its prose, moved out of
the source verbatim. Each section is headed by the line of code the comment sat above, spelled
as it is in the source, so the heading is the grep string that finds the code.

## Module

The `RunnerPreflight` shell probe: the command it runs, how long it may take, and its execution
through a [`Runner`].

INV-23's non-slotted half. The probe means the same for both runners — the container runner
executes the identical request inside the recorded image — so nothing here is host-specific and
nothing here performs an effect: the spawn belongs to whichever `Runner` the caller passes, which
for `host-v1` is the parent module's `HostRunner::run`.

The request constructor, `shell_probe_request`, stays in the parent on purpose.
`runner::contract::tests::every_production_runner_request_is_built_by_its_roles_builder` and
`runner::contract::tests::a_command_is_assembled_in_one_production_place_per_role` pin
`RunnerRequest`'s and `ShellKind::spec`'s one production construction site per role by file, and
`src/runner/host.rs` is the pinned file. Construction is pinned; execution is not.

## `#![forbid(`

**This child states its own lint level.** A Rust lint level is scoped by the module tree, not the
file, so an out-of-line child of `src/runner/host.rs` would otherwise inherit that file's
`#![allow(clippy::disallowed_types)]`. Nothing here reaches a governed primitive, so all three
governed lints are forbidden, and this module takes no `effects/allowlist.toml` row: that file
records allowances, and this module has none.
`runner::container::tests::every_child_module_of_the_container_funnel_states_its_own_lint_level`
walks `src/runner/host/` and checks this.

## `pub const SHELL_PROBE_COMMAND: &str = "exit 0";`

The command the `RunnerPreflight` shell probe runs.

`decisions.sequential_substrate.runner`: "the RunnerPreflight shell probe (the recorded shell
executing `exit 0` through the Runner …)". Not `true`, not `--version`: `exit 0` is a builtin of
every shell [`ShellKind`] names, so the probe tests the shell and not a program beside it.

## `pub const SHELL_PROBE_TIMEOUT: Duration = Duration::from_secs(10);`

A shell that has not run `exit 0` in ten seconds is unavailable for pre-flight's purposes, whatever
it is doing.

## `pub fn run_shell_probe(`

Execute the shell probe through `runner`. Availability cannot be established by inspection, only by
a spawn. `gates::shell_available` is a `PATH` check and stays one: it answers "is there a file with
that name", a different question from "does this shell run a command".

### Errors

[`UpstrokeError::Refused`]. The contract's `expected_failures_refusals[3]`: "a failing shell probe
-> returned pre-flight error to the caller (TopologyRun classifies it: creator error before P5b on a
fresh run; refusal before any recovery event on resume)". Classification is the caller's; this
returns the error.

## `if output.output_limited {`

`output_limited` means the supervisor terminated the owned process tree for exceeding the bounded
stdout or stderr capture allowance (see `output_limited` in `docs/internals/agent/proc.md`). The
limit can be observed during the final drain of a child that has already exited 0, so this is
checked on its own rather than through the exit code: such a probe did not run `exit 0` to
completion, and a shell that printed 16 MiB in answer to `exit 0` is not the shell pre-flight is
certifying either way.
