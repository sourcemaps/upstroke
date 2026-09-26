# `src/runner/host/counters.rs`

Extended notes for [`src/runner/host/counters.rs`](../../../../src/runner/host/counters.rs).

The code is the authority for what it does; this file is the whole of its prose, moved out of
the source verbatim. Each section is headed by the line of code the comment sat above, spelled
as it is in the source, so the heading is the grep string that finds the code.

## Module

The three thread-local cells behind the host runner's observables --
`program_resolutions()`, `program_searches()` and
`containment_establishments()` (see [`host.md`](../host.md)) -- declared
here rather than in `src/runner/host.rs`, which moved them on 2026-09-26
(PR #325) and imports them back.

`src/runner/host.rs` is the Process funnel and allows
`clippy::disallowed_types`, and no macro is invoked outside a function body
in a file that does not forbid every governed lint
(`effects::tests::every_macro_invocation_where_a_governed_lint_is_not_forbidden_is_inside_a_function_body`):
what a macro writes there is an item no census reads, under an allowance
that lets it hold an effect. This module forbids all three, so the one
`thread_local!` that declares the cells writes its items where nothing can
be lowered. The cells are `pub(super)`: the funnel and its children --
`naming.rs` increments `SEARCHES`, `proof` increments `ESTABLISHMENTS` --
read them through the funnel's import, as they did when the funnel declared
them.
