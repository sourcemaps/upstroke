---
id: G7-ACCEPTANCE-M5-HISTORICAL-REFUSAL
severity: P2
disposition: deferred
category: compatibility
pr: TBD
reviewed_sha: 30e0e316acb17ed2117202f1ffe93c88e3e808ee
location: src/topology/schema.rs:123
provenance: undetermined
first_bad:
guard: a reviewed compatibility and support policy for schema-3 builds made after PR3, with the supported remediation or migration it names or an explicit scoped change to the historical refusal contract, and guard evidence suited to that policy
---

## Failure sequence

The schema-compatibility contract promises that a schema-4 log is refused **explicitly** from
PR3's binaries onward. Only already-released schema-3 binaries may refuse it generically. The
module's notes say the same thing. In docs/internals/topology/schema.md, line 32 says a schema
above the ceiling refuses explicitly, and lines 46-47 say a binary built before PR12 has a
ceiling of 3 and still refuses a schema-4 log explicitly. That explicit refusal is
`TopologyLogUnreadable` (src/topology/schema.rs:123). It names the schema and the ceiling, and
says that no upgrade path exists. Gate 7's literal historical check (M5 in its acceptance map)
found a development build where that promise did not hold:

    a schema-4 run log, complete or parked, whose second line is a `task_dispatched` event
    -> is given to `status` or `resume` of a schema-3 development build made after PR3
    -> PR3 added the header-first decoder (`probe_header`), but wiring it into the production
       reader was outside PR3's scope, so that build parses the log's events before its
       compatibility check is reached
    -> it exits 1 with a generic parse error: unknown variant `task_dispatched` at line 2
    -> the operator is told a line is invalid, not that the log is a schema-4 topology run this
       build cannot read and can never upgrade into

Eight historical executions are recorded. In four of them that development build ran status and
resume, each over a complete and a parked schema-4 log. The other four used a released schema-1
build, and they end at line 1 with a missing `tactus_version` field. All eight exited 1
and refused the log. All eight are recorded as not credited and not a compatibility refusal, and
this finding changes none of them. The released schema-1 build's error is a separate matter and
is not part of this finding. The development build has not been shown to have been released, so
the contract's allowance for generic refusal by already-released binaries cannot be assigned to
it. For each call, the recorded comparison of lasting files and refs showed no change. That
comparison does not show that no transient effect occurred, or that every descendant process
settled.

This is a historical contract/implementation mismatch, not a regression in current code. The
current schema components are accepted. Those are header-first dispatch, schemas 1-3 routed to the
legacy engine, schema 4 routed to the topology, explicit refusal above the ceiling, and the legacy
upgrade guard refusing a transition to schema 4 or above. The location marks the explicit refusal
the historical build never reached, and the finding alleges no defect at that line. The statement
at lines 46-47 of the notes holds for the schema module's own selection. The recorded development
build's status and resume never reached that selection.

**Why this is a deferred P2.** On 2026-11-09 the owner accepted this historical limitation for
Gate 7's verdict, as an acceptance exception, and directed that it be recorded as a P2 finding.
That exception is the only thing that makes it nonblocking. The underlying obligation stays open
and is deferred here: a supported remediation or a scoped contract change, as below. The exception
is not new evidence, it credits none of the eight recorded executions, and the contract's text is
unchanged.

## What the change that takes this up should do

A commit made now cannot change what an already-built executable does. A fix to current code
cannot close this, and neither can relabelling or crediting the eight recorded executions after
the fact. Closure needs all of the following:

- **A reviewed compatibility and support policy.** It says whether schema-3 builds made after PR3
  are supported, and what someone running one against a schema-4 log should expect.
- **One of the following, under that policy:**
  - an actionable, supported remediation or migration for the affected builds; or
  - an explicit, scoped change to the historical contract that says from which builds explicit
    refusal is actually guaranteed. It must reach every statement of that promise, including
    lines 46-47 of the module notes.
- **Guard evidence appropriate to that policy.** It must exercise full schema-4 logs. A
  header-only refusal, a simulated lower ceiling in current code, or another executable passing
  cannot stand in for the recorded full-log negatives.

This finding does not direct re-running the eight executions, repairing an immutable binary,
shortening fixtures, or a broader historical campaign.
