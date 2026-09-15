# o3-attribution — the attribution vocabulary: the working record

The record of the pull request that lands the engine half of the 2026-09-01 decision *a runtime
question is a discovery until convicted* (obligations O3 and O2 of the private record
`2026-09-01-discovery-vs-defect-taxonomy.md`, whose public half is reproduced verbatim in §15
below). Kept on the branch so that a successor session inherits what was decided and why; written
before the work and updated as each piece lands, so that at any commit on this branch the record is
true. It is **not** a design document: `DESIGN.md` and the contract stay the authority, and a
sentence here that disagrees with either is a defect in this file.

**Branch:** `feature/o3-attribution-vocabulary`, cut from `origin/master` at
`8b28944f9607447448f5d9b9950ee49596073e58` — the merge commit of pull request #287, one pull
request past the PR10 merge `caf6bed0f9f5f749ed9ecb99fb44bfb3e08fb2ac` the goal was written
against (`git diff --stat caf6bed0..8b28944f` touches `src/agent/proc.rs`, its notes,
`CHANGELOG.md`, `design/15` and two finding files, and nothing this pull request touches:
`phase0/caf6bed0-to-base-diffstat.txt`).

**Where this file lives, and why.** As PR9's and PR10's records
(`reviews/2026-09-08-pr9-record.md`, `reviews/2026-09-12-pr10-record.md`): the owner ruled on
2026-09-08 that a slice's working record does not belong at the repository root, and
`docs/internals/` holds only module notes (`validate-internals-notes.sh` N2). So it sits beside the
gate reports in `reviews/`, dated. It also holds the public half of the contract (§15), because the
directory that half was staged for, `decisions/`, was retired on 2026-09-03 (R2).

**Evidence.** Every figure quoted here is in a saved file under
`/home/ubuntu/o3-attribution-evidence/<sha>/` on the build box, cited by path relative to that
directory. `<sha>` names the directory, not always the head measured: the base's directory,
`8b28944f…`, holds Phase 0's measurements at the base and the runs of Phases 1–6, each made on the
base plus that phase's uncommitted edits (or, for a phase's mutations, on the previous phase's
commit plus the mutation), and each log's `head:` line says which; the per-head directories
`fcfedc75…`, `3b44fc5d…` and `e03f7eae…` hold the gate runs at those heads and round 1's evidence
under `e03f7eae…/round1/`.

## 0. Status

| Step | State |
|---|---|
| 0 base measured, readings taken, record opened | **done** — this commit |
| 1 the vocabulary (`src/events/mod.rs`): `QuestionAttribution`, the two fields, `discovered`, `convicted`, `effective_attribution`; the legacy emitters unclassified; the census offer and the fixtures through the constructors; the mutations | **done** — §6; the full suite green through the wrapper, the three mutations each killed by the tests named for them |
| 2 the answer file (`src/interaction.rs`): the attributed answer record, the writer-side refusal, the schema-4 refusal executed | **done** — §7; the full suite green through the wrapper, four mutations each killed by the tests named for them; the guest run of the path-shaped tests is owed at the head the body records (§13) |
| 3 the decoder fixture (`src/topology/events.rs`, `src/topology/fold/tests.rs`): an attributed record through the informational-tolerance path; the fold untouched | **done** — §8; the full suite green through the wrapper, two mutations each killed by the tests named for them |
| 4 the internals notes, held both ways by `test-internals-notes.sh` | **done** — §9; `test-internals-notes.sh` and `test-docs-consistency.sh` green from the worktree root |
| 5 the design (O2): §5, §12, §23.1, each citing this record | **done** — §10; `test-docs-consistency.sh` green; `design/15`, `README.md`, `MAINTAINING.md` and `design/25` untouched |
| 6 the findings-ledger file | **done** — §11; `test-pr-policy.sh` and `test-pr-ledger-evidence.sh` green, the row validated by both validators |
| 7 the ten gates on this box, the guest, this record, the draft pull request | **done on this box** — the ten gates green at `fcfedc75` (§12) and at `3b44fc5d` and `e03f7eae`, the body's to report; the guest evidence is CI's Windows leg at the pushed head, by the orchestrator's ruling (§13); the draft is #290, its number set in the finding file by `e03f7eae` (§11); round 1 in §16 |
| R1 the round-1 repair of `e03f7eae` | **done** — §16: B1–B7 fixed, each its own commit; F1 filed as `PR290-R1-APPEND-DOES-NOT-ENFORCE-THE-WRITE-INVARIANT`; every mutation re-run at the round's code head; the ten gates at the round's final head are the body's to report |

## 1. What this pull request is, from the contract

The contract's obligation **O3** (the vocabulary) and **O2** (the design), landed as one pull
request off `master`, plus one findings-ledger file (Phase 6, §11) — and, since round 1, a second,
F1 (§16), so the diff adds two. In the contract's words, O3: *"Two fields on
the shared `DesignDefect` record (`src/events/mod.rs`), in the `decline_halts_run` mold —
`#[serde(default, skip_serializing_if = "Option::is_none")]`, `None` meaning "written before the
taxonomy""* — `attribution: Option<QuestionAttribution>` (`DiscoveredHole | DesignDefect`) and
`citation: Option<String>`; *"the answer *file* gains the optional attribution, never the
`question_answered` transaction, whose unknown fields are refused by construction"*; *"Structural
rule, enforced at the single writer and assumed by every projection: `Some(DesignDefect)` with
`citation: None` is invalid to write, and a reader treats a citation-less conviction as a discovery
— derived, never re-decided"*; *"The fold is untouched (`Derived::None`, fold.rs:1135);
`TOPOLOGY_EVENT_KINDS` stays 24 with 21 transactions; the neutral projection still drops the record;
the legacy writer is not modified, so legacy projections stay byte-identical (PR2 invariant) and its
records read as unclassified"*; and *"adds the decoder fixture: an attributed record read by the
informational-tolerance path"*. O2: the three `DESIGN.md` passages — §5's defect loop, §12's
pre-filter bullet, §23.1's refinement metric — amended with the O2 bullet's text, *"and touching
nothing else"*; *"§15's event list is unchanged; the wire tag stays."*

What the contract places elsewhere, and this pull request does not build: the topology's production
emitter of the record and the answer-ingest into schema 4 (*"the first topology code that emits
it"*, which PR9 was expected to be and was not — §4), the hazard map's attribution axis (O5, v0.3),
and any change to the `question_answered` transaction. §14 says so again, as what is not claimed.

## 2. What the tree already had at `8b28944f`, measured by reading

- `DesignDefect { question: QuestionId, context: String, answer: String }` at
  `src/events/mod.rs:473`, derived `Serialize`/`Deserialize` with no `deny_unknown_fields`, so an
  older reader tolerates added columns (`phase0/serde-molds.txt`, `phase0/file-hashes-at-base.txt`).
  The `decline_halts_run` mold is `QuestionAnswered` at `:464`–`:470`; the snake_case enum mold is
  `BudgetKind` at `:504`–`:508`.
- The construction-site census (`phase0/census-grep-base.txt`, `phase0/census-grep-caf6bed0.txt`,
  identical listings; classified in `phase0/census-classification.txt`): `git grep -nP
  'DesignDefect\s*\{'` matches 35 lines, 32 under `src/` and 3 under `docs/internals/`. Of the 32:
  one struct definition, two enum-variant declarations, fourteen match patterns, seven enum-variant
  constructions each wrapping a struct literal, and **eight struct-literal construction sites** of the
  record itself:

  | site | class | this pull request |
  |---|---|---|
  | `src/engine/coordinator.rs:1037` | legacy emitter (schema 3, live ingest) | `attribution: None, citation: None` |
  | `src/engine/resume.rs:585` | legacy emitter (schema 3, resume repair) | `attribution: None, citation: None` |
  | `src/events/log/tests.rs:67` | serialisation fixture: `defect()`, fed to the byte comparison in `the_legacy_append_is_byte_identical_to_the_pre_move_writer` | `None, None`, pinned literally by that test (R1) |
  | `src/events/mod.rs:1886` | serialisation fixture: the round-trip corpus of `every_event_kind_round_trips` | `None, None`, pinned literally by that test, with a discovered and a convicted sibling beside it (R1) |
  | `src/topology/census.rs:1630` | behaviour fixture: the census offer (a test-only fold-census candidate) | `DesignDefect::discovered` (R1) |
  | `src/topology/events.rs:2073` | serialisation fixture: the canonical corpus's `every_kind()` | `None, None` (R1) |
  | `src/topology/events.rs:4361` | serialisation fixture: `canonical_events()` | `None, None` (R1) |
  | `src/topology/fold/tests.rs:8452` | behaviour fixture: the fold's `every_kind()` | `DesignDefect::discovered` (R1) |

  The goal counts nine at `caf6bed0` "including the census offer". This derivation gives eight; the
  ninth line the grep matches that is neither a pattern nor a variant declaration is the struct's own
  definition, `src/events/mod.rs:473`, which constructs nothing. Recorded as a reading (R4), not a
  question.
- `TOPOLOGY_EVENT_KINDS: [&str; 24]` at `src/topology/events.rs:1264` and
  `TOPOLOGY_TRANSACTION_KINDS: usize = 21` at `:1291`
  (`phase0/kinds-and-fixtures-topology-events.txt`). `design_defect` is the last of the three
  informational kinds: `is_transaction()` at `:1323` is `!matches!(self, CapacitySnapshot |
  PoolExhausted | DesignDefect)`, and `every_kind_is_represented_exactly_once_and_the_list_agrees`
  (`:2091`) asserts the 24, the 21 and the 3.
- The fold applies the record as nothing: `src/topology/fold/start.rs:163` answers
  `Ok(Derived::None)` and `src/topology/fold/apply.rs:65` matches it to `{}`
  (`phase0/fold-sites.txt`, with both files' blob hashes at the base:
  `apply.rs` `b6bfa1c2ca60aa0907c4eaec800175cc6df55ea2`, `start.rs`
  `e8396e8dcb020b9165dcd1b395e8dd53a813c49b`).
- The fixtures whose serialised bytes must not move: `canonical_events()` at
  `src/topology/events.rs:4056` (the `design_defect` entry at `:4359`–`:4368` is
  `legacy(serde_json::to_value(DesignDefect { .. }))`) and
  `every_event_serializes_to_exactly_its_independently_written_payload` at `:4373`; the round-trip
  corpus in `src/events/mod.rs` (`:1885`); `defect()` in `src/events/log/tests.rs:65`, which
  `the_legacy_append_is_byte_identical_to_the_pre_move_writer` (`:732`) appends through the moved and
  the pre-move writer and compares on disk; the fold's `every_kind()` at
  `src/topology/fold/tests.rs:8338`; the census offer at `src/topology/census.rs:1627`. Note what
  the canonical corpus pins for `design_defect`: its expected value is computed by serialising the
  same struct (`to_value`), so a change to the struct's serialisation would move both sides of that
  assertion together. The independent pin of the pre-taxonomy bytes is Phase 1's literal-JSON test.
- The schema-4 transaction that must gain nothing: `QuestionAnswered4` at
  `src/topology/events.rs:531` carries `#[serde(deny_unknown_fields)]`, and so does its `Answer4` at
  `:518` (`phase0/deny-unknown-fields.txt`, `phase0/decoder-error-text.txt`). The decoder's own
  unknown-field text, `"unknown field `{path}` in a record embedded in a schema-4 transaction
  payload"` (`:59`), is `strict::checked`'s, for legacy records embedded in transactions through
  `deserialize_with`; `QuestionAnswered4` refuses through serde's `deny_unknown_fields` directly.
  Which text a `question_answered` payload carrying an `attribution` key produces is executed and
  quoted in Phase 2.
- The answer channel: `upstroke answer` (`src/answer.rs`) turns a reply into `crate::ir::Answer`
  (`src/ir.rs:347`, internally tagged on `answer`: `answered { text }`, `declined`, `unanswered`)
  and writes it with `interaction::write_answer` (`src/interaction.rs:96`), which stages and
  publishes through `rundir::stage_answer` and `rundir::publish_answer` (generic over
  `T: Serialize`); `interaction::read_answer` (`:105`) reads it back through
  `rundir::ingest_answer`. `src/answer.rs`'s `Answered` struct is the command's in-memory result and
  is never serialised. The legacy engine's `EventLogAnswers` reads answers through `read_answer`,
  and `coordinator::ingest_answer` (`src/engine/coordinator.rs:1006`) writes the schema-3
  `question_answered` event carrying that `Answer` and then the unclassified `design_defect`.
- The topology has **no production emitter** of the record: the seven enum-variant constructions
  above are the two legacy emitters and six fixture construction sites; `src/engine/topology/run.rs`'s
  `ingest_answers` (`:787`) appends `question_answered` (`Answer4`) and no `design_defect`. The
  contract's "Measured" section said the same of `3c09f6e`, and it is still true at the base.
- `status` renders the record as `design defect recorded for {question}`
  (`src/status/render.rs:162`–`:164`), and no test pins that line
  (`git grep -n 'design defect recorded' -- src docs` finds the one source line).
- The three notes: `docs/internals/events/mod.md` §`DesignDefect {` (line 148: *"§5: every
  question that reaches a human at runtime is a design-phase defect, logged as one so the designer
  prompt can learn from it"*), `docs/internals/topology/events.md` §`DesignDefect {` (line 1418:
  *"Informational: a question routed to the designer rather than execution"*) and its
  `is_transaction` section (line 1438: *"an informational record with an extra column costs nothing
  to ignore"*), `docs/internals/answer.md`. Two more notes carry the retired §5 sentence:
  `docs/internals/engine/coordinator.md:636` (*"§5: a question that reached a human at runtime is,
  by definition, a design-phase defect"*) and `docs/internals/interaction.md` says nothing of the
  attribution. Markers at the base: `phase0/notes-markers.txt`.
- The three design passages (`phase0/design-defect-mentions-outside-src.txt`):
  `design/05_design_work_unit_lifecycle.md:14` (*"**The defect loop:** every question that reaches
  the human at runtime is, by definition, a design-phase defect. …"*),
  `design/12_design_interaction_model.md:5` (*"… and every one that does is logged as a
  `design_defect`."*), `design/23_design_risks.md:37`–`:39` (*"Every `design_defect` is
  attributable to a story and aggregable per sprint: … a Definition of Ready with a failure
  signal."*). `design/15_design_event_log_resume_run_layout.md:107` lists `design_defect` in the
  event list and is pinned by `src/export.rs`'s `include_str!` tests; it is not touched.
- The baseline at the base, through the wrapper on this pull request's private target
  (`phase0/baseline-build.log`, `phase0/baseline-test.log`; the `Compiling upstroke v0.1.0
  (/srv/worktrees/o3-attribution)` line names this worktree): the library's `2595 passed; 0 failed;
  77 ignored` in 81.29 s, the binary's `10 passed`, the example's `0 passed`, exit `0`
  (2026-09-14, 23:49:25Z–23:50:47Z). The same counts pull request #287's body records for the merge
  commit's parent.
- The build target. `/mnt/ramtarget` (a 48 GiB tmpfs) was at 100 % when this session's first build
  ran — `rustc-LLVM ERROR: IO failure on output stream: No space left on device`
  (`phase0/baseline-build-attempt1-enospc.log`; the occupants in `phase0/ramtarget-full.txt`: the
  other sessions' directories, and this session's own 677M partial target of that failed first
  attempt, `ramtarget-full.txt:38`, which `phase0/target-redirect.txt` records removing). Following the `iso-fix-g5-c` precedent another session set minutes
  earlier, `/mnt/ramtarget/iso-impl_o3-attribution` and `/mnt/ramtarget/iso-o3-attribution` (the
  name `w1-eight-iso` derives from the worktree's basename) are symbolic links to
  `/home/ubuntu/disktarget/iso-impl_o3-attribution` (`phase0/target-redirect.txt`). The wrapper
  invocation the brief prescribes is unchanged; only where its bytes land moved. Nothing of another
  session's was touched.

## 3. Readings taken on contract ambiguities

Each is a place where the contract, the design and the code together do not determine the answer.
The contract sentence, the reading, the alternative rejected, and what a later consumer would find.

### R1 — the legacy writer, "every construction site", and "every existing serialisation fixture"

The contract: *"the legacy writer is not modified, so legacy projections stay byte-identical (PR2
invariant) and its records read as unclassified"*, and *"The post-PR9 topology writer always writes
`Some`"*. The goal (`/home/ubuntu/orch-o3-attribution/GOAL.txt`) binds two sentences at once:
*"two constructors, discovered and convicted, the latter requiring a non-empty citation, used at
every construction site on master (git grep them; nine at the PR10 merge commit caf6bed0,
including the census offer)"* and *"every existing serialisation fixture is byte-identical"*.

**Reading** (as landed in round 1; the first landing converted four fixtures to `discovered` and
so moved two serialised payloads the second sentence protects — record lens finding 1, contract
lens F2, resolved by the orchestrator's ruling in `repair-290-r1.md` B1). The two schema-3 emitters
— `src/engine/coordinator.rs:1037` and `src/engine/resume.rs:585` — keep writing unclassified
records: they gain `attribution: None, citation: None` and nothing else, and are not routed
through the constructors, which by construction never write `None`. Of the six remaining sites in
§2's table, a **serialisation fixture** is one whose test asserts its serialised bytes or feeds it
to a byte comparison, and it keeps its pre-taxonomy bytes, constructed in the literal form with the
two fields `None` and pinned by a literal assertion of its three-field payload: `defect()` in
`src/events/log/tests.rs` (fed to both writers of
`the_legacy_append_is_byte_identical_to_the_pre_move_writer`, which now opens with the pin), the
round-trip corpus of `every_event_kind_round_trips` in `src/events/mod.rs` (pinned in that test
before its loop, with a `discovered` and a `convicted` sibling beside the pre-taxonomy entry so the
attributed shapes round-trip through an `Event` too), and the canonical corpus pair in
`src/topology/events.rs` (`every_kind()` and `canonical_events()`, pinned since round 2 in
`every_event_serializes_to_exactly_its_independently_written_payload` before its loop — the
corpus's own `design_defect` entry is computed by serialising the struct, so without the pin that
test compared two values that moved together, which the round-2 fix-check lens found, S1; the
attributed cases are Phase 3's separate fixtures). Three pins, one per test. A **behaviour fixture** is one whose test exercises the census arms
or the fold and never its bytes, and it goes through the constructors as the goal's "including the
census offer" requires: the census offer in `src/topology/census.rs` and the fold's `every_kind()`
in `src/topology/fold/tests.rs`, both `discovered`. So the first sentence holds of every site that
stands for a writer's output, and the second holds of every fixture whose bytes any test compares.

**Alternative rejected.** Routing the legacy emitters through `discovered`, which would stamp every
schema-3 record a discovery and move the legacy projections' bytes; constructing the serialisation
fixtures through `discovered`, which is what the first landing did — the byte comparison and the
round-trip test could not see it, because both compute the expected bytes from the same changed
value, which is why each pin is a literal. **What a later consumer finds missing:** nothing — a
schema-3 log reads as unclassified, which the contract's hazard-map accommodation already designed
for.

### R2 — where the Public half lives and what the design cites

The contract stages its public half *"as `upstroke:decisions/2026-09-01-discovery-vs-defect-taxonomy.md`
in its own pull request after the G2 gate's declared pass, with the `DESIGN.md` amendment (O2's exact
text) in the same change"*. `decisions/` was retired on 2026-09-03 (`DESIGN.md` "Retired records":
*"They were retired so that the design is the one place a rule lives"*), and `CLAUDE.md` says the
design section changes in the same pull request with no separate record.

**Reading.** The public half's landing text goes verbatim into §15 of this record, and the three
amended passages cite this record by path, `reviews/2026-09-14-o3-attribution-record.md`. No row is
added to the retired-records table (it lists records that existed and were retired; this one never
existed there), and no other design passage is touched. The staged text says PR9 adds the fields;
this pull request does (§4), and the note under §12 says so rather than editing the staged text.

**Alternative rejected.** Recreating `decisions/` for one file, against the retirement; or citing
the private record, which a public reader cannot open.

### R3 — the wire spelling of `QuestionAttribution`

The contract writes the values as `discovered_hole` and `design_defect` throughout, and the field
type as `Option<QuestionAttribution>` with variants `DiscoveredHole | DesignDefect`. **Reading:**
`#[serde(rename_all = "snake_case")]` on the enum, `BudgetKind`'s mold (`src/events/mod.rs:504`),
giving exactly those two spellings; `Copy`, `Serialize`, `Deserialize`, `PartialEq`, `Eq`, and a
`Display` that writes the wire spelling, for the projection that shows it (R7).

### R4 — the census count

See §2: eight struct-literal construction sites by `git grep -nP 'DesignDefect\s*\{'` at the base
and at `caf6bed0`, against the goal's nine. The listing is saved, the classification is saved, and
the pull request replaces or annotates all eight; the ninth is read as the definition line. If the
goal counted something else, the saved listing is what to check it against.

### R5 — what the answer file's attributed record is

The contract: *"the answer *file* gains the optional attribution, never the `question_answered`
transaction"*, and *"a conviction only when the answer channel carries one"*. The brief asks which
type that is: `src/answer.rs`'s `Answered` is never serialised; what `interaction::write_answer`
puts on disk is `crate::ir::Answer`.

**Reading.** The file's record is a new type beside its writer, `interaction::AnswerRecord`:
the `ir::Answer` flattened (`#[serde(flatten)]`), plus `attribution: Option<QuestionAttribution>`
and `citation: Option<String>`, both `#[serde(default, skip_serializing_if = "Option::is_none")]`.
On disk an answered record is therefore `{"answer":"answered","text":…,"attribution":…,"citation":…}`
— the attribution sits on the answered record, as the brief says, and a file written before this
change, or by `upstroke answer` today, is byte-identical. Three consequences decide it over adding
the fields to `ir::Answer::Answered`:
1. **The legacy engine never sees the attribution.** `read_answer` reads the `Answer` as the base
   did (deserialised as `ir::Answer`, tolerant of columns it does not know), so the schema-3
   `question_answered` event (which embeds `ir::Answer`) and the questions payload keep their
   bytes whatever the file carries, and the legacy `design_defect` stays unclassified — which is
   what *"the legacy writer is not modified … its records read as unclassified"* requires. With the
   fields on `ir::Answer`, an attributed file ingested by a schema-3 run would carry the attribution
   on the wrong event, the one the contract says never carries it.
2. **A declined answer can carry a ruling.** The record is written for declined answers too
   (`coordinator.rs:1041`, `answer: "declined"`), and the contract's rule 1 attributes *every*
   question that reaches a human; a field on the `Answered` variant alone could not attribute a
   decline.
3. **The future ingest reads one type.** The schema-4 ingest that is not in this slice reads
   `read_answer_record`, maps the `Answer` half to `Answer4` (which has no room for the attribution,
   by `deny_unknown_fields`) and constructs `DesignDefect::discovered` or `::convicted` from the
   other half.

**The writer-side rule** is enforced where the file is written: `write_answer`, the one writer,
which takes an `AnswerRecord`, refuses `attribution: Some(DesignDefect)` with no citation (absent
or blank) before staging anything, so no `.partial` is left and no file is published.
`upstroke answer` writes `AnswerRecord::unattributed` through it, so the command's files are
unchanged. No CLI flag mints a conviction in
this pull request: that is a §18 change (`upstroke answer <question-id> [--option N | --text "…"]`)
the brief keeps out of scope, and §14 says so. **What a later consumer finds missing:** the
command-line spelling of a ruling; the file format it will write is this one.

**Alternative rejected.** Adding the fields to `ir::Answer::Answered` (`git grep -n
'Answer::Answered' -- src` at the base matches 51 lines in eleven files —
`e03f7eae…/round1/b6-answer-answered-grep-at-base.txt` — a count of matches, `Answer::Answered
{ .. }` patterns that tolerate added fields included, not a census of what would change; the
legacy leak above; no attribution on a decline).

### R6 — a blank citation is no citation at the reader too

The contract: *"a reader treats a citation-less conviction as a discovery — derived, never
re-decided"*, and `convicted` refuses *"an empty or whitespace-only citation"* (the brief).
**Reading:** the reader applies the same rule as the writer — `Some(DesignDefect)` whose citation is
absent **or blank** (`trim().is_empty()`) reads as `EffectiveAttribution::Discovered`
(`DiscoveredHole` through `.attribution()`) — so a record no constructor could have written (raw
JSON with `"citation": ""`) reads the way the writer's refusal implies. The derivation is one
function, `EffectiveAttribution::derive(stored, citation) -> EffectiveAttribution`, with the
variants `Unclassified`, `Discovered` and `Convicted { citation }`, used by
`DesignDefect::effective_attribution` and by `AnswerRecord::effective_attribution` alike. **Alternative rejected:** reading a blank citation as a conviction, which would
let a hand-edited file convict without citing.

### R7 — `status` shows the attribution

`src/status/render.rs:162` renders the record as `design defect recorded for {question}`. The
brief: decide and record whether it shows the attribution; if it does, through the reader method.
**Reading:** it does. A projection that reads the record and prints "design defect" for a ruled
discovery would restate the retired presumption. The arm matches `effective_attribution()`, which
returns an `EffectiveAttribution`: for `Unclassified` (a pre-taxonomy record, stored `None`) the
line is unchanged, byte for byte, so legacy projections stay identical (PR2 invariant);
`Discovered` prints `question {q} attributed: discovered_hole`; `Convicted { citation }` prints
`question {q} attributed: design_defect, citing {citation}`; a record whose stored value is
`design_defect` with no citation prints the discovery line, because the derivation answers
`Discovered` for it and the projection matches nothing else. **Alternative rejected:** leaving the line as it is for every record,
which reads a discovery as a defect.

### R8 — which of the eight sites the constructors reach

Restated from R1 for the census: of the eight sites, the two legacy emitters are excluded by the
contract's own sentence, the four serialisation fixtures (the canonical pair, `defect()`, the
round-trip corpus entry) by the goal's byte-identity sentence, and the two behaviour fixtures (the
census offer, the fold's `every_kind()`) go through the constructors. A reviewer counting
constructor call sites among the eight will find two, plus the attributed fixtures Phase 1's
round-trip siblings and Phase 3 add.

### R9 — which notes change

The brief names three (`events/mod.md`, `answer.md`, `topology/events.md`). `interaction.md`
changes too, because `src/interaction.rs` is where the answer file's type and writer live (R5);
`engine/coordinator.md`'s §`self.emit(EventBody::DesignDefect {` section changes, because its
sentence restates the presumption §5 retires and a note that disagrees with `DESIGN.md` is a defect
in the note (`docs/internals/README.md`); `status/render.md` gains the arm R7 changes. Two other
notes mention the record and need no change: `docs/internals/engine/tests.md:1685` (a test's
planted answered question whose `DesignDefect` append a resume repairs) and
`docs/internals/events/log/tests.md:202` (the informational kinds transcribed from the topology's
list); the search is in `e03f7eae…/round1/b7-searches.txt` (§16).

## 4. Findings this change closes or files

- **Files** (Phase 6): the topology vocabulary has declared `design_defect` unclassified since PR9
  while no topology code emits it yet, and O3 was pinned to a slice (PR9) whose contract never
  carried it. P3, `docs-contract`, `pre_existing`, deferred: what remains after this pull request is
  the emitter and the answer-ingest into schema 4, which the contract places with the first topology
  code that emits the record.
- **Closes:** nothing in `findings/`. The two open findings that name the answer channel —
  `P2_security-trust_202608310844_answer-staging-path-component-unvalidated.md` and
  `P2_correctness_202609081944_a-typed-answer-reaches-no-agent-in-schema-4.md` — are neither
  touched nor resolved: the first is about the path component, which this pull request does not
  change, and the second is about the schema-4 ingest this pull request does not build.

## 5. Measurements

Each with the file the figure lives in, under `/home/ubuntu/o3-attribution-evidence/<sha>/`.

- **Base:** `8b28944f9607447448f5d9b9950ee49596073e58` = `origin/master` at the cut
  (`8b28944f…/phase0/base.txt`). `caf6bed0` resolves to
  `caf6bed0f9f5f749ed9ecb99fb44bfb3e08fb2ac` (`phase0/caf6bed0-resolves.txt`).
- **Census:** 35 grep hits at each sha, 8 construction sites (`phase0/census-grep-base.txt`,
  `phase0/census-grep-caf6bed0.txt`, `phase0/census-classification.txt`).
- **Kinds:** `[&str; 24]` and `21` (`phase0/kinds-and-fixtures-topology-events.txt`).
- **Baseline at the base:** library `2595 passed; 0 failed; 77 ignored`, binary `10 passed`,
  example `0 passed`, exit `0` (`phase0/baseline-test.log`; the compile in
  `phase0/baseline-build.log`).

## 6. The vocabulary (Phase 1)

**What landed** (`src/events/mod.rs`, after `QuestionAnswered`):

- `QuestionAttribution { DiscoveredHole, DesignDefect }` — `Copy`, `Serialize`, `Deserialize`,
  `#[serde(rename_all = "snake_case")]` (R3), with a `Display` that writes the wire spelling.
- `EffectiveAttribution<'a> { Unclassified, Discovered, Convicted { citation: &'a str } }` — what a
  reader derives from the stored pair, and the only thing a projection reads.
  `EffectiveAttribution::derive(stored, citation)` is the one derivation: `None` → `Unclassified`;
  `Some(DesignDefect)` with a citation that is present and not blank → `Convicted`; every other
  `Some` → `Discovered` (so a citation-less or blank-cited conviction reads as a discovery, R6, and
  a discovery with a stray citation stays a discovery). `attribution()` maps it back to the
  `Option<QuestionAttribution>` the brief describes: `None`, `DiscoveredHole`, `DesignDefect`.
- `cited(&str) -> bool` — the one spelling of "a citation is not blank", used by the constructor
  and the derivation.
- `UncitedConviction`, a `thiserror` unit error: *"a design_defect conviction cites the
  design-phase checklist item or precedent that was available and unapplied; no citation, no
  conviction"*.
- `DesignDefect` gains `attribution: Option<QuestionAttribution>` and `citation: Option<String>`,
  both `#[serde(default, skip_serializing_if = "Option::is_none")]`, appended after `answer` so a
  record written without them is byte-identical to the schema-3 writer's.
  `DesignDefect::discovered(question, context, answer)` writes `Some(DiscoveredHole)`, `None`;
  `DesignDefect::convicted(question, context, answer, citation) -> Result<Self,
  UncitedConviction>` refuses a blank citation through the `Result`, never by panic;
  `effective_attribution(&self) -> EffectiveAttribution<'_>` is the reader method.
- The two legacy emitters, `src/engine/coordinator.rs:1044` and `src/engine/resume.rs:589`,
  gain `attribution: None, citation: None` and nothing else (R1; `phase1/fixture-diffs.txt`, the
  "legacy emitters" hunks).
- The two behaviour fixtures — the census offer (`src/topology/census.rs`) and the fold's
  `every_kind()` (`src/topology/fold/tests.rs`) — go through `discovered`. The four serialisation
  fixtures keep the pre-taxonomy literal, `attribution: None, citation: None`: the canonical corpus
  pair in `src/topology/events.rs` (its hunks are in `phase1/fixture-diffs.txt`, `4 +`, the two
  pairs of `None` lines and nothing else), `defect()` in `src/events/log/tests.rs` and the
  round-trip corpus of `every_event_kind_round_trips` in `src/events/mod.rs` (both converted to
  `discovered` in the first landing and returned to the literal in round 1, each with a literal pin
  of its three-field payload; R1).
- `src/status/render.rs` (R7): the `DesignDefect` arm matches `effective_attribution()`.
  `Unclassified` prints the line it always printed; `Discovered` prints
  `question {q} attributed: discovered_hole`; `Convicted { citation }` prints
  `question {q} attributed: design_defect, citing {citation}`.
- `effects/wrappers.toml`: the six new externally reachable functions of `src/events/mod.rs`
  (`attribution`, `cited`, `convicted`, `derive`, `discovered`, `effective_attribution`) are
  classified `effect_free`, because
  `effects::tests::every_externally_reachable_fn_of_a_legacy_or_shared_module_is_classified`
  refuses an unclassified name (`phase1/full-1.log`, the one failure before the classification;
  `phase1/effects-1.log`, 171 passed after it). This is a path `MAINTAINING.md` step 7 names as
  an instrument; the pull request body says so under its gate-control paragraph, and it is the
  orchestrator's to read against the delegation rule, not this record's.
- The fold is untouched: `src/topology/fold/apply.rs` and `src/topology/fold/start.rs` have the
  blob hashes §2 recorded at the base (`phase1/fixture-diffs.txt`, last two lines).

**Tests**, each existing exactly once (`phase1/fixture-diffs.txt` does not list them; `git grep
-c "fn <name>("` is 1 for each — the run in `phase1/targeted-2.log` names them all):

| test | what it proves |
|---|---|
| `events::tests::the_attribution_is_spelled_in_snake_case_on_the_wire` | `discovered_hole` / `design_defect` on the wire and in `Display`; other spellings refused |
| `events::tests::a_discovered_record_carries_its_attribution_and_no_citation_through_json` | the round trip of `discovered`, against a literal JSON string with `attribution` and no `citation` key |
| `events::tests::a_convicted_record_carries_its_citation_through_json` | the round trip of `convicted`, against a literal JSON string with both keys |
| `events::tests::an_unclassified_record_serialises_to_its_pre_taxonomy_bytes` | a `None`/`None` record inside an `Event` serialises to the literal pre-change line; a pre-taxonomy payload reads as `Unclassified`, never as a discovery |
| `events::tests::convicted_refuses_an_empty_or_blank_citation` | `""`, `" "`, `"   "`, `"\n"`, `"\t \n"` are `Err(UncitedConviction)`; a citation is `Ok` |
| `events::tests::a_conviction_without_a_citation_reads_as_a_discovery` | raw JSON the constructor cannot produce — `design_defect` with no `citation`, with `""`, with `"  \n"` — keeps its stored value and reads as `Discovered`; a cited one reads as `Convicted`; a discovery with a stray citation reads as `Discovered` |
| `engine::tests::the_legacy_ingest_writes_an_unclassified_design_defect` | the live schema-3 emitter, driven through `run_harness` with a scripted answer: the one `design_defect` line on disk has exactly the keys `answer`, `context`, `question`, and reads as `Unclassified` |
| `engine::tests::the_resume_repair_writes_an_unclassified_design_defect` | the resume emitter, driven through the decline-prefix mold (`truncate_log_after(…, "question_answered")`, then `resume_with`): the record it appends has the same three keys and reads as `Unclassified` |

**The existing serialisation fixtures keep their bytes, and which test bodies are in the diff.**
`every_event_decodes_from_its_independently_written_payload` and
`every_kind_is_represented_exactly_once_and_the_list_agrees` in `src/topology/events.rs` are
unmodified and pass (`phase1/full-2.log`, and every later full run). Three test bodies **are** in
the diff, each by a literal pin — two added in round 1 (`e03f7eae…/round1/`, B1), the third in
round 2 (§17, B1): `every_event_serializes_to_exactly_its_independently_written_payload` asserts,
before its loop, that the corpus's `design_defect` payload is exactly the three-key object
(`q-design-0001`, the Ünicode context, `rescope`);
`the_legacy_append_is_byte_identical_to_the_pre_move_writer` opens with
`serde_json::to_value(defect("q-1"))["data"] == {"question":"q-1","context":"context Ünicode","answer":"answer"}`,
and `every_event_kind_round_trips` asserts the same three-key shape of its `q-1` entry before its
loop and carries the two attributed siblings. Executed: at `e03f7eae` (the fixtures still
`discovered`) the log test's pin fails with `"attribution": String("discovered_hole")` in the
payload (`round1/b1-before/pin-on-converted-fixture.log`, exit `101`); at the round-1 head both
pins pass, and the mutation that converts `defect()` back to `discovered` fails the log test's pin
(`round1/mutations/M10-…`, §16).

**Mutations, executed** (`phase1/mutations/summary.txt`, one `.diff` and `.log` per mutation; the
file restored from a pristine copy and its SHA-256 checked equal after each):

| mutation | tests that fail | tests that keep passing, and why |
|---|---|---|
| M1 `skip_serializing_if` removed from `attribution` | `an_unclassified_record_serialises_to_its_pre_taxonomy_bytes`, `the_legacy_ingest_writes_an_unclassified_design_defect`, `the_resume_repair_writes_an_unclassified_design_defect` | `every_event_serializes_to_exactly_its_independently_written_payload` and `the_legacy_append_is_byte_identical_to_the_pre_move_writer` pass: both compute their expected bytes by serialising the same struct, so they cannot see this mutation — which is why the literal-JSON test exists |
| M2 the reader returns the stored value unconditionally | `a_conviction_without_a_citation_reads_as_a_discovery` (`left: Convicted { citation: "" }`, `right: Discovered`) | the rest |
| M3 `convicted` accepts an empty citation | `convicted_refuses_an_empty_or_blank_citation` | the rest |

M2's first application did not compile (a missing comma in the mutated arm, `M2-…` first entry in
the summary); it was re-applied well-formed and is the row above.

**Runs** (all through the wrapper on the private target): `phase1/targeted-2.log` (48 passed),
`phase1/fmt-2.log` (clean after `cargo fmt`), `phase1/clippy-1.log` (`-D warnings`, clean),
`phase1/full-2.log`: library `2603 passed; 0 failed; 77 ignored` in 93.64 s, binary `10 passed`,
exit `0` (2026-09-15, 00:10:08Z–00:11:42Z) — 2595 at the base plus the eight tests above.

## 7. The answer file (Phase 2)

**What landed** (`src/interaction.rs`, R5):

- `AnswerRecord { answer: Answer (flattened), attribution: Option<QuestionAttribution>, citation:
  Option<String> }` — what `answers/<question-id>.json` holds. The two optional fields are in the
  same serde mold as the record's (`#[serde(default, skip_serializing_if = "Option::is_none")]`),
  so a record without a ruling serialises to the answer's own bytes.
  `AnswerRecord::unattributed(answer)`, `::discovered(answer)`,
  `::convicted(answer, citation) -> Result<Self, UncitedConviction>` (the same `cited` rule as
  `DesignDefect::convicted`), and `effective_attribution()` through the same
  `EffectiveAttribution::derive`.
- `write_answer(dir, id, &AnswerRecord)` — the one writer of the file, and the writer-side rule:
  `attribution: Some(DesignDefect)` with a citation that is absent or blank is refused with
  `UpstrokeError::Refused` (*"answer {id} is not written: a design_defect conviction cites …; no
  citation, no conviction"*) before anything is staged, so no `.partial` and no file. Its callers
  moved with it: `src/answer.rs` writes `AnswerRecord::unattributed(answer)` (the command's files
  are unchanged), and the two test callers in `src/interaction.rs` and the one in
  `src/engine/tests.rs` wrap their `Answer` the same way. No new effectful wrapper, so
  `clippy.toml`'s disallowed list is untouched; `write_answer` keeps its `effectful` row.
- `read_answer_record(dir, id) -> Result<Option<AnswerRecord>, _>` reads the whole record and is
  the one reader that validates the two attribution columns; `read_answer` deserialises
  `ir::Answer` directly, as the base did, so a column the answer does not know is ignored as it
  always was and the legacy engine's `EventLogAnswers` and `coordinator::ingest_answer` see exactly
  what they saw before (R5, consequence 1). Both go through one private `read_answer_as<T>`. In the
  first landing `read_answer` read through `AnswerRecord` and returned its `answer` half, which
  refused a file such as `{"answer":"unanswered","citation":7}` that the base tolerated, and
  `Run::sweep_answers` propagated the `Parse` error before task selection — the regression lens's
  P2, round 1 B2. The lens's recipe (the scheduler-spin test's answer file replaced by that line)
  executed: at the base `8b28944f`, exit `0`, parked; at `e03f7eae`, exit `101`,
  `resume: Parse { … invalid type: integer `7`, expected a string at line 1 column 35 }`
  (`e03f7eae…/round1/b2-recipe/{base,head-before-fix}.log` and the `.diff` of each); after the fix,
  exit `0` again (`round1/b2-recipe/head-after-fix.log`, §16). The recipe turned permanent is
  `engine::tests::a_legacy_answer_file_with_a_foreign_column_still_parks_rather_than_erroring`.
- `effects/wrappers.toml`: `convicted`, `discovered`, `effective_attribution`,
  `read_answer_record`, `unattributed` classified `effect_free` in the `src/interaction.rs` entry.
- `src/topology/events.rs` gains the executed refusal (below). `QuestionAnswered4` and `Answer4`
  are untouched.

**Tests**, each existing exactly once:

| test | what it proves |
|---|---|
| `interaction::tests::an_attributed_answer_file_is_read_back_with_its_ruling` | a `convicted` record is written, its file has exactly the keys `answer`, `attribution`, `citation`, `text`, `read_answer_record` reads it back equal and `Convicted`, and `read_answer` returns the answer alone; a `discovered` decline round-trips too; no `.partial` is left |
| `interaction::tests::an_unattributed_answer_file_keeps_the_bytes_the_writer_always_wrote` | for `answered`, `declined` and `unanswered`, `to_string(&AnswerRecord::unattributed(a)) == to_string(&a)` and the file on disk is `to_string_pretty(&a)` plus a newline; a hand-written pre-change file reads as `Unclassified` |
| `interaction::tests::the_writer_refuses_a_conviction_without_a_citation` | `design_defect` with no citation, an empty one and a blank one are refused by the writer with the message above, naming the question, and the directory stays empty; `AnswerRecord::convicted` refuses the empty string, a blank and a whitespace-with-newline citation (its parameter is a `String`, so "absent" is not a value it can take). The first landing exercised the constructor with the one input `" \n"` — the record lens's finding 5; round 1 B4 added the empty and blank inputs, and the lens's mutation M11 (`if !citation.is_empty() && !cited(&citation)`), which the one-input test survived at `e03f7eae`, fails the empty-string assertion now (§16) |
| `interaction::tests::a_conviction_without_a_citation_in_the_file_reads_as_a_discovery` | a hand-written `design_defect` with no `citation` keeps its stored value and reads as `Discovered`; `read_answer` still returns the answer |
| `topology::events::tests::a_question_answered_transaction_refuses_an_attribution_key` | the canonical `question_answered` payload decodes; with `attribution` (either spelling) or `citation` added on the payload the decoder answers exactly `unknown field `attribution`, expected one of `key`, `question`, `answer`, `via`` (and likewise for `citation`); added inside `answer` it answers exactly `unknown field `attribution`, expected `option_index` or `binding_override`` |
| `engine::tests::a_legacy_answer_file_with_a_foreign_column_still_parks_rather_than_erroring` (round 1) | a schema-3 run parked on a question; its answer file written by hand as `{"answer":"unanswered","citation":7}`; the resume runs and parks again, as the base did, instead of erroring on the column |

Those two quoted strings are serde's own, from `QuestionAnswered4`'s and `Answer4`'s
`deny_unknown_fields` (§2); `strict::checked`'s *"in a record embedded in a schema-4 transaction
payload"* is the text for a legacy record embedded through `deserialize_with`, which
`question_answered` does not carry. Executed in `phase2/targeted-1.log` and `phase2/full-1.log`;
the test pins the strings, so a change to either text fails it.

**Mutations, executed** (`phase2/mutations/summary.txt`, one `.diff` and `.log` each; both files
restored from pristine copies and their SHA-256 checked equal after each):

| mutation | tests that fail | tests that keep passing |
|---|---|---|
| M4 the writer's refusal removed | `the_writer_refuses_a_conviction_without_a_citation` | the other seven named |
| M5 `#[serde(flatten)]` removed from `AnswerRecord.answer` | `an_unattributed_answer_file_keeps_the_bytes_the_writer_always_wrote`, `an_attributed_answer_file_is_read_back_with_its_ruling`, `a_conviction_without_a_citation_in_the_file_reads_as_a_discovery` | `answers_survive_the_trip_through_a_file` and `answer::tests::an_answer_lands_where_the_engine_will_find_it` — they write and read through the same type, so the shape moves under them unnoticed, which is why the literal-bytes test exists |
| M6 the reader drops `attribution` (`skip_deserializing`) | `an_attributed_answer_file_is_read_back_with_its_ruling`, `a_conviction_without_a_citation_in_the_file_reads_as_a_discovery` | the rest |
| M7 `deny_unknown_fields` removed from `QuestionAnswered4` | `a_question_answered_transaction_refuses_an_attribution_key`, and the existing `a_transaction_refuses_an_unknown_field_and_an_informational_record_ignores_it` | the rest |

**Runs** (through the wrapper on the private target): `phase2/targeted-1.log` (197 passed over
`interaction::`, `answer::`, `effects::`, the refusal test and the unanswered-answer engine test),
`phase2/fmt-1.log` (clean after `cargo fmt`), `phase2/clippy-1.log` (`-D warnings`, clean),
`phase2/full-1.log`: library `2608 passed; 0 failed; 77 ignored` in 88.02 s, binary `10 passed`,
exit `0` (2026-09-15T00:17:01Z–2026-09-15T00:19:09Z) — Phase 1's 2603 plus the five tests above.

**Not built here, by the brief:** the schema-4 answer-ingest that would read
`read_answer_record` into a `design_defect`, and a command-line spelling of a ruling (§14).

## 8. The decoder fixture (Phase 3)

**What landed.** In `src/topology/events.rs`'s tests, beside `canonical_events()` and not in it:
`attributed_design_defects()`, two fixtures each pairing a body built through a constructor with an
**independently written** payload (a `json!` literal, not `to_value` of the struct — the pin the
corpus's own `design_defect` entry lacks, §2): a discovery (`DesignDefect::discovered`, payload
with `attribution: "discovered_hole"` and no `citation`) and a cited conviction
(`DesignDefect::convicted`, payload with both keys), each with the `EffectiveAttribution` it must
read as. In `src/topology/fold/tests.rs`, one test that drives both records through the fold.
`canonical_events()` and `every_kind()` keep their pre-taxonomy `design_defect` entry (R1);
`src/topology/fold/apply.rs` and `src/topology/fold/start.rs` are byte-identical to the base
(`git diff 8b28944f -- src/topology/fold/apply.rs src/topology/fold/start.rs` is empty at every
commit of this branch; §2's blob hashes).

**Tests**, each existing exactly once:

| test | what it proves |
|---|---|
| `topology::events::tests::an_attributed_design_defect_serializes_to_its_independently_written_payload` | each attributed body serialises to exactly its literal payload |
| `topology::events::tests::an_attributed_design_defect_reads_through_the_informational_path` | each payload decodes to its body; `kind()` is `design_defect`; `is_transaction()` is false; `effective_attribution()` is `Discovered` / `Convicted { citation }`; with an unknown column (`"Ünknown Column  "`) added to `data` the record still decodes, equal to the body — *"an informational record with an extra column costs nothing to ignore"* — while the same column on the canonical `question_answered` payload is refused with `unknown field `Ünknown Column  ``; and, in passing, `TOPOLOGY_EVENT_KINDS.len() == 24` and `TOPOLOGY_TRANSACTION_KINDS == 21` |
| `topology::fold::tests::an_attributed_design_defect_folds_to_no_derived_state` | on a fold with a merged task, `plan_transition` of the discovery and of the conviction each yields a delta whose `derived` is `Derived::None`, and `apply_delta` leaves `state()` equal to what it was |

The invariants the contract names are held by the existing assertions, unmodified:
`every_kind_is_represented_exactly_once_and_the_list_agrees` (24 kinds, 21 transactions, the
informational three named), `every_event_serializes_to_exactly_its_independently_written_payload`
(the corpus covers all 24 and the `design_defect` payload is the pre-taxonomy one), and the type of
`TOPOLOGY_EVENT_KINDS` itself, `[&str; 24]`. `a_transaction_refuses_an_unknown_field_and_an_informational_record_ignores_it`
already sweeps every kind with an unknown field; the new test adds the attributed shapes it cannot
see.

**Mutations, executed** (`phase3/mutations/summary.txt`, `.diff` and `.log` each; files restored
from pristine copies and their SHA-256 checked equal):

| mutation | tests that fail | tests that keep passing |
|---|---|---|
| M8 the topology's `DesignDefect` payload made strict (`#[serde(deserialize_with = "strict::field")]`) | `an_attributed_design_defect_reads_through_the_informational_path`, and the existing `a_transaction_refuses_an_unknown_field_and_an_informational_record_ignores_it` | the serialisation pin, the corpus tests, the fold test |
| M9 `fold/start.rs` derives `Derived::Answer(QuestionOrigin::Admission)` from a `design_defect` | `an_attributed_design_defect_folds_to_no_derived_state`, by its `Derived::None` assertion on the delta | the rest: `a_delta_carries_the_exact_event_it_was_checked_against` supplies `run_started`, a dispatch, a question and a `capacity_snapshot` and never a `design_defect`, so its inputs cannot reach the mutated arm (the first landing said `apply.rs` was the reason — record lens finding 5); and the fold test's own state assertion cannot see M9 either, because `apply.rs` matches the record to `{}` whatever the delta derived. M9 is witnessed by the derived-state assertion alone; no further test is needed |

M9 is a mutation of the fold and was restored; the fold is untouched at every commit.

**Runs**: `phase3/targeted-1.log` (53 passed over `topology::events::tests` and the fold's
informational tests), `phase3/fmt-1.log` (clean after `cargo fmt`), `phase3/clippy-1.log`
(`-D warnings`, clean), `phase3/full-1.log`: library `2611 passed; 0 failed; 77 ignored` in
90.83 s, binary `10 passed`, exit `0` (2026-09-15T00:29:45Z–2026-09-15T00:31:57Z) — Phase 2's 2608 plus the three tests above.

## 9. The internals notes (Phase 4)

Six notes files change, the three the brief names and the three R9 adds; every changed module
keeps its single `Extended notes:` pointer and no other prose (§13), and every new production item
— type, field, function — has a section headed by its source line (`docs/internals/README.md`'s
grep-string rule; the check per item is in `e03f7eae…/round1/b7-searches.txt`). Of the new tests,
`docs/internals/topology/events.md` carries sections for the three whose purpose is not their
name; the other new tests have none, which the convention asks of no test —
`validate-internals-notes.sh` checks markers and backlinks (N1–N4), not sections:

| notes file | what changed |
|---|---|
| `docs/internals/events/mod.md` | the `DesignDefect {` variant section restated for the attribution loop; new sections for `QuestionAttribution` and its variants, `EffectiveAttribution` with `derive` and `attribution`, `cited`, `UncitedConviction`, `pub struct DesignDefect {`, its two new fields, and `discovered`, `convicted`, `effective_attribution` |
| `docs/internals/topology/events.md` | the `DesignDefect {` section: the attributed payload, the informational-tolerance path, the refused transaction, no emitter yet; the `is_transaction` section names the two columns as the columns it describes; sections for `a_question_answered_transaction_refuses_an_attribution_key`, `attributed_design_defects`, and the two attributed-fixture tests |
| `docs/internals/answer.md` | the module section says what the file holds, that this command writes it unattributed, and where a ruling can and cannot arrive; a section for the record the command now builds |
| `docs/internals/interaction.md` | sections for `AnswerRecord`, its two fields and four methods; `write_answer` restated as the single writer and the writer-side rule; `read_answer_record`; `read_answer` restated as the answer half the legacy engine reads |
| `docs/internals/engine/coordinator.md` | the `self.emit(EventBody::DesignDefect {` section no longer restates the presumption §5 retires: the schema-3 writer writes the record unclassified, not through the constructors |
| `docs/internals/status/render.md` | a section for the `DesignDefect` arm: three lines, read through the reader method, the legacy line unchanged |

Gates from the worktree root: `bash .github/scripts/test-internals-notes.sh` — `internals notes:
150 marker(s), 150 notes file(s), all resolve both ways`, `41 cases passed`, exit `0`
(`phase4/test-internals-notes.log`); `bash .github/scripts/test-docs-consistency.sh` — `PASS`,
exit `0` (`phase4/test-docs-consistency.log`).

## 10. The design (Phase 5)

Three passages, and nothing else under `design/` or in `DESIGN.md` (R2). Each cites this record by
path.

- `design/05_design_work_unit_lifecycle.md:14` — the paragraph that began **The defect loop** is
  replaced by the contract's O2 text beginning **The attribution loop**, verbatim through *"at
  exactly the rate the checklist absorbs what execution discovers"*, followed by the citation
  sentence (*"Decided 2026-09-01; the verdict, the engine's obligations and the vocabulary that
  carries the attribution are recorded in `reviews/2026-09-14-o3-attribution-record.md`."*).
- `design/12_design_interaction_model.md:5` — the pre-filter bullet's last clause, *"and every one
  that does is logged as a `design_defect`."*, becomes the contract's *"and every one that does is
  logged with its attribution: `discovered_hole` by default, `design_defect` only by citation
  (§5; `reviews/2026-09-14-o3-attribution-record.md`)."*
- `design/23_design_risks.md:37`–`:43` — the sentence *"Every `design_defect` is attributable to a
  story and aggregable per sprint: … a Definition of Ready with a failure signal."* is split as the
  O2 bullet says: **question volume** (`question_raised` counts, attributable to a story and
  aggregable per sprint, unchanged and free — the badly refined story still parks on a recorded
  question naming what refinement failed to settle) and **conviction rate** (the share ruled
  `design_defect`, each citing the checklist item refinement skipped — a Definition of Ready
  failure signal that cannot be gamed by punishing discovery). The bullet's other sentences are
  unchanged.

`design/15_design_event_log_resume_run_layout.md:107` still lists `design_defect` in the event
list, and the wire tag stays. `git diff --stat fb92164c -- design/15_design_event_log_resume_run_layout.md
README.md MAINTAINING.md design/25_design_export_decisions_schema.md` is empty, so the sentences
`src/export.rs` pins are untouched. No other file copies the amended sentences: `git grep -n
'defect loop\|attributable to a story\|logged as a `design_defect`' -- . ':!reviews/'` finds
the three lines above and nothing else (the run in this phase's shell, before the edit). Gate from
the worktree root: `bash .github/scripts/test-docs-consistency.sh` — `PASS`, exit `0`
(`phase5/test-docs-consistency.log`).

## 11. The findings-ledger file (Phase 6)

`findings/P3_docs-contract_202609150036_the-topology-vocabulary-declared-design-defect-unclassified-since-pr9.md`,
id `O3-DESIGN-DEFECT-UNCLASSIFIED-SINCE-PR9`, in `findings/README.md`'s shape: `P3` (no consequence
can be shown — no topology log with the record exists, §2), `docs-contract`, `deferred`,
`provenance: pre_existing`, `reviewed_sha` the base, `location` `src/topology/events.rs:1261` (the
`DesignDefect { data: DesignDefect }` variant at the base), `first_bad` PR9's merge commit
`74da2cbbd24c55f7aed7f3593162981a11720f79` (resolved from `git log --merges --grep 'pull request
#249' origin/master`), `guard` the slice that adds the topology's emitter. Its failure sequence is
what a reader of a topology log would have concluded since PR9 — every record the retired
presumption's defect, no way to tell a discovery from a conviction, no writer for either — and what
this pull request changes; it stays open because the emitter and the schema-4 answer-ingest are
still owed (§14).

The `pr:` field was blank until the pull request existed — the orchestrator's answer to question
1 rules out predicting the number — and `e03f7eae`, the second push that answer authorised for
that one-line change, set it to `290`, the number `gh pr create` reported.

The pull request body's ledger row, validated with the finding staged
(`phase6/ledger-row.txt`; `phase6/draft-body-for-validation.md` through `validate-pr-body.sh`,
exit `0`, and `validate-pr-ledger-evidence.sh <head>` with the finding committed on a throwaway
commit, exit `0`: `phase6/validate-pr-body.log`, `phase6/validate-pr-ledger-evidence.log`). The
gates from the worktree root: `bash .github/scripts/test-pr-policy.sh` — `PR policy fixtures
passed`, exit `0` (`phase6/test-pr-policy.log`); `bash .github/scripts/test-pr-ledger-evidence.sh`
— `PR ledger evidence fixtures passed`, exit `0` (`phase6/test-pr-ledger-evidence.log`).

## 12. The ten gates on this box

At `fcfedc755f99c39a9178630dd70eb58352c9c268`, the first landing's code-complete head. The three
commits after it up to `e03f7eae` are this record's own and the finding file's `pr:` line (blank
at `3b44fc5d`, `290` at `e03f7eae`: `git diff --stat fcfedc75 3b44fc5d -- .
':!reviews/2026-09-14-o3-attribution-record.md'` is that one file, `1 insertion(+), 1
deletion(-)`, and the same command with `e03f7eae` in place of `3b44fc5d` prints nothing, the
field having gone `290` → blank → `290`); round 1's commits follow (§16), and the gates at round
1's final head are the body's to report.
Logs under `fcfedc755f99c39a9178630dd70eb58352c9c268/gates/`: `box-load.log`,
`w1-eight-iso.log`, `eight-logs/NN-<name>.log` (a copy of `~/eight-logs/fcfedc7/`),
`test-pr-ready-audit.log`.

`~/bin/w1-eight-iso /srv/worktrees/o3-attribution`, bare, from a clean worktree (`git status
--porcelain` empty), after `git ls-files -z -- src build.rs Cargo.toml Cargo.lock | xargs -0 touch`
so the crate was compiled again rather than taken from an earlier build; `/tmp/w1-eight.lock` was
free when it started, the box's load average was 38.80 (`box-load.log`). The target base it
derived, `/mnt/ramtarget/iso-o3-attribution`, is the symbolic link §2 describes, so its bytes are
on disk and its cache is this pull request's own. It printed, 2026-09-15 00:41:54Z–00:45:18Z:

| step | result |
|---|---|
| `cargo fmt --check` | PASS, 1 s |
| `cargo clippy --all-targets --all-features -- -D warnings` | PASS, 25 s |
| `cargo test --all-targets --all-features` | PASS, 133 s: library `2611 passed; 0 failed; 77 ignored` in 90.35 s, binary `10 passed`; `03-test.log` opens `Compiling upstroke v0.1.0 (/srv/worktrees/o3-attribution)` and runs `/mnt/ramtarget/iso-o3-attribution/slot1/debug/deps/upstroke-1f05c71869a3a005` |
| `cargo +1.85.0 check --locked --all-targets --all-features` | PASS, 27 s |
| `test-release-record.sh` | PASS |
| `test-pr-policy.sh` | PASS, 16 s |
| `test-pr-ledger-evidence.sh` | PASS |
| `test-docs-consistency.sh` | PASS |
| `test-internals-notes.sh` | PASS, 2 s |

`ALL 9 PASS at fcfedc7`, exit `0`. Then, from the worktree root, `bash
.github/scripts/test-pr-ready-audit.sh`: `test-pr-ready-audit: ok`, exit `0`
(`test-pr-ready-audit.log`). The library count is Phase 3's 2611: the phases after it added no
test.

The same ten gates are run again at the head the body records, and that run is the body's to
report, under `<that head>/gates/` in the evidence directory.

## 13. The Windows guest

**The local guest run was not made.** The orchestrator's answer to question 1
(`/home/ubuntu/orch-o3-attribution/answers/impl_o3-attribution-1.md`, 2026-09-15 ~01:00Z) rules
option C: CI's `test (winguest)` leg at the pushed head is this pull request's guest evidence,
recorded by the orchestrator in the body at merge time; nothing on the guest that is not this
session's is created, deleted, moved or written, and no space is freed — freeing it means deleting
other sessions' data, which the orchestrator escalated to the owner. The one exception the answer
allows, decided by a measurement: immediately before the push `C:` is measured once more, read-only,
the output saved under the evidence directory for the pushed head, and only if it then has 12 GB or
more free is a tree of this pull request's own (`C:\upstroke-o3`, its own target, from a bundle of
the head) made and the path-shaped tests run there. No sentence in this record, the body, the notes
or the finding file claims a guest run was made.

**Why there is no room**, each figure from a saved file under the evidence directory:

- `orch/guest-disk-20260915T005753Z.txt` (the orchestrator's read-only measurement, 00:57:53Z):
  `C:` 0.46 GB free, 118.46 GB used; `D:`, `E:` and `F:` 0 GB free; two `cargo` processes (started
  23:56:02 and 00:03:43) and three `upstroke-8bdbea19d48fcc33` test binaries (23:56:02, 00:03:44,
  00:08:54), none of them ours.
- `guest/guest-listing-20260915T010021Z.txt` (this session's read-only listing, 01:00:32Z, nothing
  created or written on the guest): `C:` 498,536,448 bytes free against 127,184,461,824 used; `D:`
  6,010,140,672 used and 0 free; `E:` 789,645,312 and 0; `F:` 387,072 and 0; two `cargo.exe` and
  eight `upstroke-8bdbea19d48fcc33` processes running. The build targets on `C:`, none this
  session's: `C:\upstroke-pr10\target` 15,122,137,284 bytes, `C:\upstroke-g5\target`
  15,160,180,955 (the G5 session's), `C:\cargo-target` 9,104,151,320, `C:\upstroke\target`
  6,730,667,875, `C:\upstroke-fix-target` 4,417,538,490, `C:\upstroke-redo-target` 3,921,409,879,
  `C:\pr9r3-target` 3,549,349,514, `C:\pr8-fix8-target` 3,530,234,008,
  `C:\upstroke-verify-target` 2,851,813,521 — 64,387,482,846 bytes across those nine, beside
  `C:\actions-runner` (the self-hosted CI runner) and the `pr*`, `upstroke-*` and `wingate-*` trees
  the listing names.
- `guest/guest-disk-and-tasks-captured-20260915T0033Z-to-0036Z.txt` (this session's earlier
  read-only capture): `C:` 498,974,720 bytes free, five `upstroke-8bdbea19d48fcc33` processes and
  two `cargo.exe`.

A build of this crate's test binary needs several GB (the nine targets above are between 2.85 GB
and 15.2 GB each), and filling the last half-gigabyte would also break the CI runner's Windows leg
for every open pull request; so no tree of this pull request's was made on the guest.

**What is path-shaped in this pull request**, and so is what CI's Windows leg will exercise:
`interaction::tests::an_attributed_answer_file_is_read_back_with_its_ruling`,
`interaction::tests::an_unattributed_answer_file_keeps_the_bytes_the_writer_always_wrote`,
`interaction::tests::the_writer_refuses_a_conviction_without_a_citation` and
`interaction::tests::a_conviction_without_a_citation_in_the_file_reads_as_a_discovery` (an answer
file under a temporary directory, its staging and its rename);
`engine::tests::the_legacy_ingest_writes_an_unclassified_design_defect` and
`engine::tests::the_resume_repair_writes_an_unclassified_design_defect` (a run directory and a git
repository); and `answer::tests::*`, unchanged but through the changed writer. The vocabulary, the
decoder fixture, the fold test and the render arm are path-free. All of these pass on Linux at
`fcfedc75` (§12). If the CI leg fails with `No space left on device`, that is the guest's condition
and the orchestrator's to handle, not a defect of this change.

## 14. What is not claimed

- **No topology emitter of the record.** The topology still writes no `design_defect`; the
  constructors are the writer-side rule for the code that will, and the decoder fixture is what such
  a record reads as. The contract's *"The post-PR9 topology writer always writes `Some`: the default
  at ingest"* describes that writer, not a writer in this tree.
- **No answer-ingest into schema 4.** `src/engine/topology/run.rs` still maps an answer to
  `Answer4` and appends `question_answered` only; reading the attribution out of the answer file
  into a record is the emitter's, above.
- **No command-line spelling of a ruling.** `upstroke answer` writes unattributed files; the file
  format that will carry a ruling is this pull request's, the flag is a §18 change (R5).
- **The hazard map's attribution axis** (O5) is v0.3's.
- **The census offer is a test fixture**, not an emitter: `src/topology/census.rs` is
  `#[cfg(test)]` machinery for the fold census, and its `design_defect` candidate going through
  `discovered` proves the fold's indifference to the attribution, nothing about production.
- **The legacy engine reads no attribution.** By R5 an attributed answer file ingested by a
  schema-3 run yields an unclassified record and an unchanged `question_answered`; the ruling in that
  file is not lost — the file stays where `upstroke answer` left it — but no schema-3 record carries
  it, which is the contract's "its records read as unclassified".

## 15. The public half of the contract, verbatim

The landing text the private record stages under its heading "Public half (staged for landing;
named identically on both sides)", reproduced without change. Its relative link to
`2026-08-27-proposals-private.md` is as staged; that record was retired with `decisions/` on
2026-09-03 and its substance is the "Retired records" section of `DESIGN.md`. Where the text says
PR9 adds the two fields, this pull request does (R2, §4).

```markdown
# 2026-09-01 — a runtime question is a discovery until convicted

**Verdict.** `DESIGN.md` §5 and §12 presumed every question that reaches the
human at runtime a design-phase defect. That presumption is retired, for every
run kind. A runtime question is logged with an attribution whose default is
`discovered_hole` — execution surfaced a decision design could not reasonably
have been expected to exhaust. Recording `design_defect` requires citing the
design-phase checklist item or existing precedent that was available and
unapplied — no citation, no conviction. Every ruled discovery becomes checklist
or precedent material, so its recurrence in a later plan is citable, and is a
defect. Convicted defects remain review material for the designer prompt; the
system still learns to need the user less, from honestly labelled data.

**The reasoning is recorded privately**, with the design work it concerns, per
[2026-08-27](2026-08-27-proposals-private.md). This record carries the verdict
and the obligations that follow from it.

## Consequences

- `DESIGN.md` is amended in the same change as this record: §5's defect loop
  becomes the attribution loop, §12's pre-filter bullet closes "…logged with
  its attribution: `discovered_hole` by default, `design_defect` only by
  citation (§5)", and §23.1's refinement metric splits into question volume
  (`question_raised` counts, unchanged) and conviction rate. §15's event list
  is unchanged.
- The `design_defect` event keeps its wire tag and its informational standing —
  one of schema 4's three informational kinds, applied by the fold with no
  derived state, dropped by the neutral projection. The tag is a historical
  name; the record carries the judgment.
- PR9 — the first topology emitter of the record — adds two optional fields in
  the `decline_halts_run` mold: `attribution` (`discovered_hole` |
  `design_defect`; absent = written before this record) and `citation` (a
  conviction's cited item; absent on discoveries). The post-PR9 writer always
  writes the attribution; a conviction arrives only through the answer
  channel's file, never as a field on the `question_answered` transaction,
  whose unknown fields are refused by construction. Writer-side rule, assumed
  by every projection: a citation-less conviction is invalid to write and
  reads as a discovery. No transaction changes; the topology event-kind list
  stays 24 with 21 transactions; the legacy writer is untouched and its
  records read as unclassified.
- Nothing lands inside the G2 evidence range, and nothing rides the G2
  promotion: `2026-08-25-checkpoint-merges.md` and the 2026-08-31 promotion
  record stay controlling and untouched. This record and its amendment land as
  their own change after the gate's declared pass.
- The 2026-08-11 design-council clause — a runtime `design_defect` traced to a
  council design needs an attributable seat — now binds the convicted subset;
  discoveries indict no seat.

## Measured

At the private record's filing head (engine tree `3c09f6e`): `design_defect` is
informational — `is_transaction()` false, folded with no derived state, dropped
by the neutral projection; the topology has no production emitter of it (the
legacy emitters write schema 3 until activation); answer-ingest is PR9's;
`DesignDefect` carries no `deny_unknown_fields`, so older readers tolerate the
added columns; the frozen parallelism packet's only `design_defect` occurrence
is the neutral projection's drop list, and `question_raised` does not occur in
it.

## Rejected

- **Keeping the blanket stamp.** Measured discovery rates make the majority
  product of deliberate exploration book as designer demerits, poisoning the
  §5 loop and the §23.1 metric with mislabeled data.
- **Splitting the regime by run kind.** The same question priced differently
  by which run raised it; the citation requirement already carries the
  calibration.
- **A new or renamed event kind inside schema 4.** Cosmetics priced as a
  change to the frozen kind list; wire tags are historical names and schema-4
  logs replay unchanged.
- **Waiting for schema 5.** PR9 would ship the first emitter under the retired
  presumption, minting exactly the mislabeled records this decision exists to
  prevent, then migrating.
```

## 16. Round 1 — the review of `e03f7eae`

Three `gpt-6-astra` lenses at `max` reviewed `e03f7eaec026a02cb5feac916dd22961794a7816`:
contract, regression and record, each `CHANGES_REQUIRED`
(`/home/ubuntu/orch-o3-attribution/reviews/r1/{contract,regression,record}.md`; the combined
comment https://github.com/sourcemaps/upstroke/pull/290#issuecomment-5673424882). CI at that head
was green on both contexts, the Windows leg included (run 34916931904). The orchestrator's brief
for the round is `/home/ubuntu/orch-o3-attribution/repair-290-r1.md`; its evidence is under
`e03f7eaec026a02cb5feac916dd22961794a7816/round1/`. Each item, what it found, what changed, and
where the execution is:

- **B1 (record 1, P1; contract F2): "every existing serialisation fixture is byte-identical" was
  false.** Four fixtures had been converted to `discovered`; two of them are serialisation
  fixtures whose payloads moved, and their tests could not see it. Resolved as R1 now reads, with
  the ruling's classes: `defect()` and the round-trip corpus entry back to the `None, None`
  literal, each pinned; the census offer and the fold fixture stay `discovered`. Executed: the pin
  on the converted fixture at `e03f7eae` fails, `"attribution": String("discovered_hole")` in the
  payload (`round1/b1-before/pin-on-converted-fixture.log`, exit `101`, the diff beside it); at
  `371525a7` both pins pass (`round1/b1-targeted.log`, 139 passed); mutation M10, `defect()`
  converted to `discovered`, fails `the_legacy_append_is_byte_identical_to_the_pre_move_writer`
  and leaves `every_event_kind_round_trips` passing, since it converts one fixture
  (`round1/mutations/M10-pinned-fixture-converted-to-discovered.{diff,log}`, exit `101`).
- **B2 (regression, P2): `read_answer` refused legacy files the base tolerated.** The lens's
  recipe — the scheduler-spin test's answer file replaced by `{"answer":"unanswered","citation":7}`
  — executed at the base (`round1/b2-recipe/base.log`, exit `0`, parked), at `e03f7eae`
  (`head-before-fix.log`, exit `101`: `resume: Parse { … invalid type: integer `7`, expected a
  string at line 1 column 35 }`), and after the fix at `8f94b356` (`head-after-fix.log`, exit `0`);
  the three diffs beside them. The fix: `read_answer` deserialises `ir::Answer` directly and only
  `read_answer_record` validates (§7); the recipe turned permanent is
  `a_legacy_answer_file_with_a_foreign_column_still_parks_rather_than_erroring`, and mutation M12
  (the reader routed back through `AnswerRecord`) fails it and it alone
  (`round1/mutations/M12-read-answer-through-the-record-type.{diff,log}`, exit `101`).
  `docs/internals/interaction.md`'s `read_answer` and `read_answer_record` sections say so.
- **B3 (contract F3; record 3): the readings named interfaces that do not exist.** R5, R6 and R7
  rewritten to `AnswerRecord`, `read_answer_record`, `write_answer`, `AnswerRecord::unattributed`,
  `EffectiveAttribution::derive` and the enum's variants. Searches, before and after, in
  `round1/b3-searches.txt`: `git grep -n -E 'AnswerFile|read_answer_file|write_answer_file|QuestionAttribution::effective' -- .`
  matched six lines of this record and, by the regex alone, the unrelated identifier
  `PlantedAnswerFiles` in `src/engine/topology/recover/tests.rs`, its notes and PR10's record
  (untouched); `grep -n '== None'` on this record matched R7's one sentence; the handover and the
  body matched neither. After the rewrite this record matches only this entry's own quotation
  of the regex (the "after" listing in the same file).
- **B4 (record 5): a test description exceeded its assertions; the M9 sentence gave the wrong
  reason.** `the_writer_refuses_a_conviction_without_a_citation` now asserts the constructor's
  refusal for `""`, `"   "` and `" \n"`; the record's row and the M9 row rewritten (§7, §8).
  Executed: the lens's mutation M11 (`if !citation.is_empty() && !cited(&citation)`) survives the
  test as it stood at `e03f7eae` (`round1/b4-before/M11-before.log`, exit `0`) and fails the
  empty-string assertion at `92b45c9d` (`round1/mutations/M11-constructor-accepts-empty.{diff,log}`,
  exit `101`, `the constructor refuses what the writer refuses: ""`).
- **B5 (record 2): the body's "46 of 1,069" was a count by test name, stated as a count by
  assertion.** The 2026-09-14 scan (`~/pr10-evidence/r10/reaper-lease-flake-scan.log`) selected
  logs by the line `… a_host_integration_reaper_holds_the_runs_cleanup_lease ... FAILED`, not by
  the assertion text; of its 46, 45 carry `the hold outlived the reaper that took it` and one,
  `~/pr10-evidence/1c5bb58c0dd7c860600ad8f27f53de7f64b2a9b7/full-suite-export.log`, fails earlier
  with `the resume settles the planted state: Io … No such file or directory`. Re-scanned on
  2026-09-15 by both selections, the command and every matching path in
  `round1/b5-flake-rescan.txt`: 1,119 logs, 57 with the test reported `FAILED`, 56 with the
  assertion text, the one difference that same export log. The body's sentence says this.
- **B6 (record 6): measurements and references not true at the head.** Corrected, each to its
  file: the baseline's start is `23:49:25Z` (`phase0/baseline-test.log`'s `date:` line; `23:49:02Z`
  was the compile's `end:`), Phase 1's full run starts `00:10:08Z` (`phase1/full-2.log`); R1's
  "four test fixtures and the census offer" is the table's two behaviour fixtures and four
  serialisation fixtures since B1; the evidence paragraph says what the base's directory holds and
  what each log's `head:` line names; §12 says what the commits after `fcfedc75` change (the
  finding's `pr:` line, not nothing); the tmpfs occupants included this session's own 677M
  partial target; the "51 sites" sentence names its saved grep and says it counts matches. The
  body: the finding's `pr:` is `290` at every head since `e03f7eae`; the attempt-1 directory is
  `eight-logs-attempt1-failed/`; the fenced numstat is the literal output of the two commands,
  leading space included.
- **B7 (record 4; contract R9 and its coverage note): two notes overstated, and two record
  claims were false.** `docs/internals/answer.md` no longer says an unattributed file is read "as
  the default" — the schema-3 engine that ingests it writes its record unclassified and the
  reader reports `Unclassified`; the topology emitter that applies the default is unbuilt.
  `docs/internals/events/mod.md`'s `pub struct DesignDefect {` section names exactly which sites
  use the literal `None, None` form (the two legacy emitters and the four serialisation fixtures)
  and which go through the constructors. R9's "No other note mentions the record" corrected to the
  two that do; §9's "every new item" narrowed to every new production item, with the per-item
  heading check saved. Searches in `round1/b7-searches.txt`: `git grep -n -i
  'design_defect\|DesignDefect' -- docs/internals` (the six changed notes and the two others), and
  for each new production item a grep of its notes file for a heading naming it.
  `bash .github/scripts/test-internals-notes.sh` after the edits: `round1/b7-test-internals-notes.log`.
- **F1 (contract, P2): the event-append path does not enforce the write invariant.** Filed, not
  fixed, as `findings/P2_correctness_202609150214_the-event-append-path-does-not-enforce-the-write-invariant.md`
  (`PR290-R1-APPEND-DOES-NOT-ENFORCE-THE-WRITE-INVARIANT`, `correctness`, `pr: 290`,
  `reviewed_sha` `e03f7eae`, `location` `src/events/mod.rs:530`, `introduced_by_feature`,
  `first_bad` `c4362105`, `deferred`), in its own findings-only commit, the last before the push,
  with the body's ledger row. Why filed: the round-1 brief's ruling, "File, do not fix". What
  the contract says, only for what it says: it rejects fold-validated convictions (*"Refusing a
  citation-less conviction at the fold makes an informational record quasi-transactional and puts
  gate weight on a record the gates deliberately ignore"*) and places enforcement thus
  (*"Enforcement belongs to the single writer and to projections' read rule"*); whether an
  append-side check is compatible with that is an open reading for the first production writer's
  slice. Round 2 corrected this entry, the file and the body, which had attributed a broader "log
  or fold layer" prohibition to the contract — the round-1 brief's wording (round-2 record 2,
  contract F3; §17). Why harmless at this head, each by the grep the file names: no path assigns the two fields after
  construction (`git grep -n -E '\.(attribution|citation)\s*=[^=]' -- src` matches nothing);
  `Some(QuestionAttribution::DesignDefect)` occurs in the constructors, the derivation, the answer
  writer's check and test literals only; the two legacy emitters write `None, None`; the topology
  has no emitter; and every projection reads the shape as a discovery.

**Every mutation recipe re-run at the round's code head**, `33a90025c5ae913b80b7f20635dd87232c3c4bba`
(the `src` tree is `92b45c9d`'s, `ed5bef5b5455516fe5ca44da3e442f32dcd4b765`: the commits between
are notes and this record; the commits after it are the record, `bb73be2b`, and the finding,
`c3688ada`). Runner `mutate.py` (in the session's scratchpad; the summary's first line names its
path, and its text is not saved): each recipe is the exact string replacement the phase sections
attribute to it, applied,
run through the wrapper on the named tests, restored, and the file's SHA-256 checked against the
pristine — `33a90025…/mutations/summary.txt`, `<name>.diff`, `<name>.log`:

| mutation | outcome at `33a90025` |
|---|---|
| M1 `skip_serializing_if` removed from `attribution` | exit `101`: the three Phase 1 failures, **and the two round-1 pins** — `the_legacy_append_is_byte_identical_to_the_pre_move_writer` and `every_event_kind_round_trips`; the corpus test passed under it at this head (`… every_event_serializes_to_exactly_its_independently_written_payload ... ok` in the log), the survivor the round-2 fix-check lens named (S1) and the round-2 pin closes (§17) |
| M2 the reader returns the stored value | exit `101`: `a_conviction_without_a_citation_reads_as_a_discovery` |
| M3 `DesignDefect::convicted` accepts an empty citation | exit `101`: `convicted_refuses_an_empty_or_blank_citation` |
| M4 the writer's refusal removed | exit `101`: `the_writer_refuses_a_conviction_without_a_citation` |
| M5 `#[serde(flatten)]` removed | exit `101`: the three Phase 2 failures, **and now** `answers_survive_the_trip_through_a_file` and `answer::tests::an_answer_lands_where_the_engine_will_find_it`, because `read_answer` reads `ir::Answer` directly since B2 and an unflattened file is not one |
| M6 the reader drops the attribution | exit `101`: the two Phase 2 failures |
| M7 `deny_unknown_fields` removed from `QuestionAnswered4` | exit `101`: the two Phase 2 failures and `an_attributed_design_defect_reads_through_the_informational_path`, whose transaction half it also breaks |
| M8 the topology's `DesignDefect` payload made strict | exit `101`: the two Phase 3 failures |
| M9 the fold derives an answer from the record | exit `101`: `an_attributed_design_defect_folds_to_no_derived_state` |
| M10 the pinned `defect()` converted to `discovered` (round 1) | exit `101`: `the_legacy_append_is_byte_identical_to_the_pre_move_writer`; `every_event_kind_round_trips` passes, its own fixture untouched |
| M11 `AnswerRecord::convicted` accepts the empty string (round 1, the lens's) | exit `101`: `the_writer_refuses_a_conviction_without_a_citation` |
| M12 `read_answer` routed through `AnswerRecord` (round 1) | exit `101`: `a_legacy_answer_file_with_a_foreign_column_still_parks_rather_than_erroring`, and it alone |
| B2's recipe (the spin test's file replaced by the foreign column), expected to pass | exit `0`: `an_answer_file_that_changes_nothing_does_not_spin_the_scheduler` passes — the witness that the fix holds |

**The gates and the guest at the round's final head** are the body's to report, under that head's
directory: the ten gates from a clean worktree, the read-only measurement of the guest's `C:`
immediately before the push (the answer-1 threshold, 12 GB, decides whether a local guest run is
made; §13), `git merge-tree --write-tree origin/master HEAD` (`origin/master` moved to `aff2b024`
during the round: two pull requests touching `findings/` and `scripts/pr-review-parse.py`, nothing
this branch touches — the merge is clean and the base is not merged in), and both body validators.
