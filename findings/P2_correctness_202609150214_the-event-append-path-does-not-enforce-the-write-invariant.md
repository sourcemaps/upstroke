---
id: PR290-R1-APPEND-DOES-NOT-ENFORCE-THE-WRITE-INVARIANT
severity: P2
disposition: deferred
category: correctness
pr: 290
reviewed_sha: e03f7eaec026a02cb5feac916dd22961794a7816
location: src/events/mod.rs:530
provenance: introduced_by_feature
first_bad: c4362105db7a11d023bcb5befe787016359222d3
guard: the topology emitter slice that builds the first production writer of the attributed record — it constructs through `DesignDefect::discovered` and `DesignDefect::convicted` and never through the struct literal, and its test appends a record built each way and reads the log back
---

## Failure sequence

`DesignDefect` (`src/events/mod.rs`, the struct at line 530 of the reviewed head) has public
`attribution` and `citation` fields and derives `Serialize` without restriction. The write
invariant of the 2026-09-01 decision — `Some(DesignDefect)` with no citation is invalid to write —
is enforced by the constructors (`convicted` refuses a blank citation) and by the answer file's
writer (`interaction::write_answer`), and nowhere on the event-append path: construct a valid
record with `DesignDefect::convicted(…, "checklist item 2")` -> set its public `citation` to
`None` -> wrap it in `TopologyEventBody::DesignDefect` -> `TopologyLine::round_trip`
(`src/events/log.rs`) serialises and deserialises it, permissive deserialisation being the
informational path's contract -> `EventLog::append_topology_hooked` checks the site and the handle
and writes the bytes -> the log holds the shape O3 declares invalid to write. The reader's derived
rule (`EffectiveAttribution::derive`) then treats it as a discovery, which satisfies the read rule
and not the write rule. Filed from the contract lens's F1 on PR #290's round-1 review
(https://github.com/sourcemaps/upstroke/pull/290#issuecomment-5673424882); its recipe is a test
`an_uncited_conviction_cannot_be_appended` beside `defect()` in `src/events/log/tests.rs`, not
executed in that review and not added here.

**Why it is filed and not fixed.** The orchestrator's brief for PR #290's round-1 repair
(`/home/ubuntu/orch-o3-attribution/repair-290-r1.md`, "File, do not fix") ruled that this pull
request files the gap and implements no append-side check. What the contract itself says, verbatim
and only for what it says: it rejects **fold**-validated convictions — *"**Fold-validated
convictions.** Refusing a citation-less conviction at the fold makes an informational record
quasi-transactional and puts gate weight on a record the gates deliberately ignore."* — and it
places enforcement thus: *"Enforcement belongs to the single writer and to projections' read
rule."* Neither sentence speaks of the event-append path. Whether a check at
`EventLog::append_topology`, or in the record's own serialisation, is part of "the single writer"
or is the fold-validation the contract rejects is an open reading, left to the slice that builds
the first production writer of the record. An earlier version of this file said the contract
"rejects validating the record on the log or fold layer"; that was the round-1 brief's wording,
not the contract's (the round-2 record and contract lenses, finding 2 and F3).

**Reachability at the reviewed head.** No production path mutates the two fields after
construction: `git grep -n -E '\.(attribution|citation)\s*=[^=]' -- src` matches nothing, tests
included; `git grep -n 'Some(QuestionAttribution::DesignDefect)' -- src` matches the two
constructors, the derivation, the answer writer's check and test literals only. The two legacy
emitters (`src/engine/coordinator.rs`, `src/engine/resume.rs`) write `None, None`; the topology
has no production emitter of the record at all (`reviews/2026-09-14-o3-attribution-record.md`
§14). And the reader rule makes the shape harmless to every projection: `status` and any reader of
`effective_attribution()` see a discovery, never a conviction. So at this head the invalid shape
can be written only by code that sets a public field on purpose, and read only as a discovery.

## What the change that takes this up should do

Build the first production writer of the attributed record — the schema-4 answer-ingest's emitter
— so that it constructs through `DesignDefect::discovered` and `DesignDefect::convicted` and
never through the struct literal, and prove it with a test that appends a record built each way
and reads the log back. Whether the append path should itself refuse the shape is the open reading
above, to be taken by that slice with the contract in hand and recorded in its record as a reading
with the two sentences quoted — not decided here. Until then the fields stay public because the
two legacy emitters and the serialisation fixtures construct the literal form.
