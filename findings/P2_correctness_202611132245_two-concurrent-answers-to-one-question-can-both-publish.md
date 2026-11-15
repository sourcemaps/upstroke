---
id: PR381-ANSWER-PUBLISH-RACE
severity: P2
disposition: deferred
category: correctness
pr: 381
reviewed_sha: b0f8fff544bf809f837a74cb1cf6df57899826d3
location: src/answer.rs:323
provenance: pre_existing
first_bad:
guard: make answer publication itself refuse an existing `answers/<id>.json` on every schema — have each publisher stage into a file of its own (unique per publisher, e.g. named with its process id and a random suffix, created exclusively) rather than the shared `<id>.json.partial`, then publish create-once from that file, through a hard link, or `renameat2(RENAME_NOREPLACE)` where the platform has it with a fallback that refuses rather than replaces; a lock serialising `upstroke answer` is not enough on its own, because it does not stop a later, serialised second answer on a schema 1–3 run, whose refusal reads only `questions/<id>.json` — witnessed by two publishers for one question started together, of which exactly one succeeds and the other is refused, and the retained `answers/<id>.json` holds the bytes of the publisher that was told it succeeded; and, on a schema 1–3 run with no engine running, by a publisher A that publishes and returns, then a publisher B that is refused, with `answers/<id>.json` still holding A's bytes
---

## Failure sequence

Found by reading the code in review, not reproduced. The sequences below follow the code; that
they are reachable in practice, and the exact outcome on each platform, are inferred.

### Two concurrent answers (schema 4, and schema 1–3)

1. Two `upstroke answer` commands, A and B, answer the same open question of one run at the same
   time. On a schema-4 run both reach `answer_open` (`src/answer.rs:320`) and both pass its only
   guard, the absent-file check at `src/answer.rs:323` (`interaction::read_answer` finds no
   `answers/<id>.json`), before either has published. (On a schema 1–3 run both pass the
   question-payload check at `src/answer.rs:47` instead; see the next sequence for why that run
   needs no concurrency at all.)
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
B reports an error for one that was published.

### Two sequential answers (schema 1–3 only)

On a legacy run the overwrite needs no concurrency. Every fresh sequential run is one: the schema
it writes is `SCHEMA_VERSION`, which is 3 (`src/events/mod.rs:25`).

1. A legacy run parks a question and the engine stops, leaving `questions/<id>.json` with
   `answer: None` and no `answers/<id>.json`.
2. Command A runs `answer` (`src/answer.rs:30`). Its only refusal of an answered question is at
   `src/answer.rs:47`, and it reads the question payload, not the answers directory: it refuses
   only when `record.answer` is set. It is not, so A publishes `answers/<id>.json` through
   `interaction::write_answer` (`src/answer.rs:66`) and exits successfully. Publishing writes only
   the answers directory; neither `answer` nor `write_answer` updates `questions/<id>.json`.
3. Command B is run afterwards — not concurrently, A has already returned. It reads the same
   unchanged payload, still `answer: None`, passes the same check, and its `fs::rename` in
   `rundir::publish_answer` replaces A's file (on Unix). B exits successfully too.
4. The payload gains the answer only when the engine ingests one (`ingest_answer` rewrites it at
   `src/engine/coordinator.rs:1255`). So the window runs from A's publish to the next ingestion,
   and while the engine is stopped it has no bound. Any number of later answers replace each
   other in it, and each reports success.
5. The next incarnation's `sweep_answers` reads B, records `QuestionAnswered` with B, and
   rewrites the payload. The log, the payload and the file all agree on B. A's command reported
   success for an answer that never took effect, and nothing tells A's operator so.

Both sequences come from one gap: the refusal is not made by the publish itself, and the publish
does not refuse an existing answer. The guard below closes both.

## Why P2

This was filed as P3 because it was read as concurrency-only: two operators, or two tools,
answering one question at once, inside the window between the absent-file check and the rename.
That holds for a schema-4 run. It does not hold for a schema 1–3 run, which is what every fresh
sequential run is, and that run's sequence is deterministic. It needs only two answers to one
parked question before the engine ingests one: a second operator, or a notifier integration, or
the same operator answering again while the engine is stopped. Neither needs to overlap the
other. The command that loses gets no error and no warning. Its success contradicts the refusal's
own sentence, "Answers are applied once", and an operator who acted on that success (told a
colleague, closed a ticket) has nothing to tell them it was undone. Those are the reasons for P2.

It is not P1. The engine's state follows the log, and on the sequential path the log, the payload
and the retained file all agree with each other. Nothing is corrupted, and the run acts on an
answer an operator really gave. On the concurrent path the run behaves as the recorded answer
says, and only the retained file on disk disagrees with the log.

## What the change that takes this up should do

Make publication itself refuse an existing `answers/<id>.json`, on every schema, so that the
destination is the check. A check made before the publish, of whatever file, leaves a gap.

First, have each publisher stage into a file of its own instead of the shared
`answers/<id>.json.partial`: a name unique to the publisher (for instance carrying its process id
and a random suffix), created exclusively so that two publishers can never write the same staged
file. Create-once alone is not enough while staging is shared: A stages A, B overwrites the shared
partial with B, A links or no-replace-renames the partial and reports success for A, B is refused,
and the retained answer is B's. Then make publication create-once from that per-publisher file, so
the second publisher is refused instead of replacing the first: publish through a hard link from
the staged file (which fails if the destination exists), or `renameat2(RENAME_NOREPLACE)` where
the platform provides it with a fallback that refuses rather than replaces.

A lock that serialises `upstroke answer` per run is not a substitute. It orders two answers, but
on a schema 1–3 run the second one, run after the first has released the lock, still reads only
the unchanged question payload and passes. That is the sequential sequence above, untouched. Only
a publish that refuses an existing destination closes both sequences.

The refusal should read as the existing "was already answered" refusal does. Witness it twice:

- **Concurrent:** two publishers for one question, started together and each writing a distinct
  answer. Exactly one succeeds and the other is refused, and the retained `answers/<id>.json`
  holds the bytes of the publisher that was told it succeeded — not merely some answer.
- **Sequential, schema 1–3, engine stopped:** publisher A publishes and returns success. Then
  publisher B, for the same question and with a distinct answer, is refused, and
  `answers/<id>.json` still holds A's bytes.
