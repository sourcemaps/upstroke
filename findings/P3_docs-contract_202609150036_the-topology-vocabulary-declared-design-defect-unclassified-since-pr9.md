---
id: O3-DESIGN-DEFECT-UNCLASSIFIED-SINCE-PR9
severity: P3
disposition: deferred
category: docs-contract
pr: 290
reviewed_sha: 8b28944f9607447448f5d9b9950ee49596073e58
location: src/topology/events.rs:1261
provenance: pre_existing
first_bad: 74da2cbbd24c55f7aed7f3593162981a11720f79
guard: the slice that adds the topology's first emitter of the record — the schema-4 answer-ingest — which constructs it through `DesignDefect::discovered` or `DesignDefect::convicted` from `interaction::read_answer_record`
---

## Failure sequence

The 2026-09-01 decision — a runtime question is a discovery until convicted; its public half is
reproduced in `reviews/2026-09-14-o3-attribution-record.md` §15 — placed its obligation O3, the
attribution vocabulary on the shared `DesignDefect` record, with "PR9, the first topology emitter
of the record", so that no topology log would ever exist in the unclassified form. PR9's slice
contract (`reviews/2026-09-08-pr9-record.md` §1) never carried O3, and PR9 merged at
`74da2cbbd24c55f7aed7f3593162981a11720f79` without an emitter: `git grep -nP 'DesignDefect\s*\{'`
at that merge's successor `caf6bed0` and at `8b28944f9607447448f5d9b9950ee49596073e58` finds two
schema-3 emitters and five test fixtures, and no topology code that writes the record
(`reviews/2026-09-14-o3-attribution-record.md` §2). So from PR9 to `8b28944f` the topology
vocabulary (`TopologyEventBody::DesignDefect { data: DesignDefect }`, `src/topology/events.rs:1261`)
declared a `design_defect` whose record had no attribution column at all -> a reader of a topology
log had to read every such record as the retired presumption's "a design-phase defect", with no way
to tell a discovery from a conviction and no writer that could have written either -> the decision's
O3 stood outstanding with nothing in the tree saying so.

What the pull request that files this changes: the record carries `attribution` and `citation`,
`None` on both reads as "written before the taxonomy" rather than as a defect, and the schema-4
decoder reads an attributed record through its informational-tolerance path
(`an_attributed_design_defect_reads_through_the_informational_path`). What it leaves, and why this
file stays: the topology still has no production emitter of the record, and the schema-4
answer-ingest (`src/engine/topology/run.rs`, `ingest_answers`) appends `question_answered` and
nothing else, so no topology log carries an attributed record yet.

## What the change that takes this up should do

Add the topology's emitter where the answer is ingested: read the answer file with
`interaction::read_answer_record`, append the `question_answered` transaction as today, and append
the `design_defect` record built through `DesignDefect::discovered` when the file carries no ruling
or a discovery, and through `DesignDefect::convicted` when it carries `design_defect` with a
citation — never a bare struct literal, so the topology's writer always writes `Some`. Prove it with
a topology run whose answer file carries a conviction and whose log then carries
`"attribution":"design_defect"` and the citation on the record and nothing of either on the
transaction.
