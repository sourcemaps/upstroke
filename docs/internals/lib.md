# `src/lib.rs`

Extended notes for [`src/lib.rs`](../../src/lib.rs).

The code is the authority for what it does; this file is the whole of its prose, moved out of
the source verbatim. Each section is headed by the line of code the comment sat above, spelled
as it is in the source, so the heading is the grep string that finds the code.

## Module

upstroke — headless orchestration engine for AI coding agents.

Copyright 2026 Cameron Lambert
SPDX-License-Identifier: Apache-2.0

Licensed under the Apache License, Version 2.0. Distributed on an "AS IS"
basis, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND; see the LICENSE and
NOTICE files at the repository root, or
<http://www.apache.org/licenses/LICENSE-2.0>.

v0.1 scope (DESIGN.md §21, steps 1–10): parse an annotated markdown plan
into the IR, resolve a routing chain per task, and execute it sequentially —
one agent subprocess per attempt, gates and read-only review over the
engine-captured diff, one commit per task, every transition an event in
`events.jsonl`. `resume`, `status`, and `answer` are folds over that log.

The capacity engine (§13) ships **read-only**: `connect` discovers the agent
CLIs and writes the pools file, `capacity` and the dry-run preview estimate
what is left and what each strategy *would* do, and budgets stop a run at a
ceiling — but nothing routes on any of it. Capacity-driven binding is v0.2.

## `#![forbid(non_local_definitions)]`

The one lint level this file states, added on 2026-09-26 (PR #325). A macro
invoked inside a function body can expand to an `impl` of a type or trait
defined outside that body, and the methods it defines are then callable
from anywhere in the crate under a name no source text spells, so no
census reads it and its body is compiled under whatever allowance the
invoking file carries. `non_local_definitions` is rustc's lint for that
`impl`; it only warns by default, and an inner `#[allow]` -- which the
macro can write itself -- lowers it below CI's `-D warnings`. `forbid`
cannot be lowered by any inner attribute (`E0453`), so every module of the
library is held to it without a reader recognising anything. Unlike a
governed lint (below), this one has no allowance anywhere in the crate for
a `forbid` here to collide with.

It does not reach a module-level `const _` initializer, which rustc exempts
from the lint; `effects::tests::every_crate_root_forbids_non_local_definitions`
records what it does and does not refuse, measured, and pins this line in
all three crate roots.

## Held to declarations

This file declares the crate's modules and nothing else, and since #318's
third round `effects::tests::a_declaring_module_holds_declarations_and_re_exports_and_nothing_else`
holds it to that shape beside `src/engine/mod.rs`: a `fn`, a constant, an
inline module, `include!` or a macro written here is a red test. The reason
is the lint level: every allowance in the crate sits below this file, so it
can neither `forbid` a governed lint (`E0453` at the first allowance) nor
usefully `deny` one (a `deny` is a level an inner `allow` lowers), and a
file that states nothing and holds no code hosts nothing -- which is what
`every_unclassified_production_file_states_each_governed_lint_or_inherits_its_forbid`
takes as the reason to pass over it.
