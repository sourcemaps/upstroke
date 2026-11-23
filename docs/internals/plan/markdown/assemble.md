# `src/plan/markdown/assemble.rs`

Extended notes for [`src/plan/markdown/assemble.rs`](../../../../src/plan/markdown/assemble.rs).

The code is the authority for what it does; this file is the whole of its prose, moved out of
the source verbatim. Each section is headed by the line of code the comment sat above, spelled
as it is in the source, so the heading is the grep string that finds the code.

## Module

Assembly: ids, kinds, dependencies, artifacts.

The last step, and the only one that mints IR. Explicit `id=` values are
reserved before any slug is derived, so a derived id never collides with
one an author wrote; duplicate explicit ids are left intact for `validate`
to report. An absent `depends=` chains a task to its predecessor in
document order, and `depends=` with no value breaks that chain
deliberately. Kinds fall back to a keyword heuristic over the title.

The sink of the DAG: fed by [`super::drafts`] and [`super::hints`], read by
nothing but the adapter itself.

## `pub(super) fn assemble(drafts: Vec<Draft>) -> Vec<Task>` › `let mut taken: Vec<String> = drafts`

Reserve explicit ids first so derived slugs never collide with them.
Explicit duplicates are left intact for validation to report.

The reservation reads each draft's annotation by reference and keeps its
own copy of each explicit id, and that copy is the point: `taken` has to
stay valid after the loop below consumes the drafts by value, and each
annotation's own id moves from there into the task it belongs to. The
registry and the task are two owners of one string, which is what the
copy says; nothing here is a borrow the checker refused.

## `pub(super) fn assemble(drafts: Vec<Draft>) -> Vec<Task>` › `let Draft {`

Each draft is taken apart, not read through, so its annotation and its
title, body and criteria move into the task. No annotation is copied:
reserving ids borrowed it, and building the task is its one owner.

## `fn unique_slug(title: &str, taken: &mut Vec<String>) -> String {`

Two copies here are owned snapshots, not borrows the checker refused.
`base` is the stable suffix template: every later round of the loop reads
it again to build `{base}-{n}`, while `candidate` is replaced, so the
first candidate has to be its own owned copy rather than a move of
`base`. The chosen slug then needs two owners: the reservation registry,
which keeps it so a later draft cannot derive the same slug again, and
the returned value, which becomes this task's id.

## `pub(super) fn collect_artifacts(tasks: &mut [Task], warnings: &mut Vec<String>) -> Vec<Artifact> {`

Artifacts come from `out=` annotations; a bare plan with a Design task
defaults to a conventions brief produced by the first one (§9).
