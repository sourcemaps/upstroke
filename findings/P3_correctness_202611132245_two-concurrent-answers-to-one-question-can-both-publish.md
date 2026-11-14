---
id: PR381-ANSWER-PUBLISH-RACE
severity: P3
disposition: deferred
category: correctness
pr: 381
reviewed_sha: b0f8fff544bf809f837a74cb1cf6df57899826d3
location: src/answer.rs:323
provenance: pre_existing
first_bad:
guard: make answer publication create-once — publish through a hard link, or `renameat2(RENAME_NOREPLACE)` where the platform has it with a fallback that refuses rather than replaces, or serialise `upstroke answer` per run under a lock — witnessed by two publishers for one question of which exactly one succeeds and the other is refused
---

## Failure sequence

Found by reading the code in review, not reproduced. The interleaving below follows the code; that
it is reachable in practice, and the exact outcome on each platform, are inferred.

1. Two `upstroke answer` commands, A and B, answer the same open question of one run at the same
   time. Both reach `answer_open` (`src/answer.rs:320`) and both pass its only guard, the
   absent-file check at `src/answer.rs:323` (`interaction::read_answer` finds no
   `answers/<id>.json`), before either has published.
2. Each stages its own `answers/<id>.json.partial` and publishes it with
   `crate::rundir::publish_answer`, a plain `fs::rename` onto `answers/<id>.json`
   (`src/rundir.rs`, reached from `interaction::write_answer` at `src/interaction.rs:144`). The
   check at step 1 is not atomic with that rename, and nothing else refuses an existing
   destination.
3. A publishes first. The engine's answer poll (`TopologyRun::ingest_answers`, or `sweep_answers`
   on a legacy run) reads A, records `QuestionAnswered` with A in the run's log, and the question
   is settled.
4. B publishes next. Its rename replaces A's file without error (on Unix a rename onto an
   existing name replaces it; on Windows the outcome depends on how `fs::rename` maps there —
   inferred, not checked). B's command reports success.
5. Result: the retained answer file holds B while the log recorded A. A resume does not ingest B,
   because the replayed log has already removed the question from the open set, so the run's state
   is A throughout; only the retained record on disk disagrees with it. (Inferred: if both
   publishes land before the engine polls, the engine ingests whichever file it reads — B — and
   A's command has reported a success that took no effect.)

Both commands also stage to the same `<id>.json.partial` name, so other interleavings exist
(inferred): if B's stage overwrites A's before A renames, A publishes B's content and B's own
rename then fails on a missing staged file, so A reports success for an answer it did not write and
B reports an error for one that was published. Every interleaving comes from the same gap, a check
that is not atomic with a publish that does not refuse, and the guard below closes all of them.

## Why P3

It needs two operators, or two tools, answering one question at once, inside the window between
the absent-file check and the rename. The engine's state follows the log, not the retained file,
so the run behaves as the recorded answer says; what goes wrong is that one command reports an
answer applied that was not, and the answer file left on disk no longer matches the log.

## What the change that takes this up should do

Make publication create-once, so the second publisher is refused instead of replacing the first:
publish through a hard link from the staged file (which fails if the destination exists), or
`renameat2(RENAME_NOREPLACE)` where the platform provides it with a fallback that refuses rather
than replaces; or serialise `upstroke answer` per run under a lock held across the check and the
publish. The refusal should read as the existing "was already answered" refusal does. Witness it
with two publishers for one question, started together, of which exactly one succeeds and the
other is refused, and the retained file holds the one that succeeded.
