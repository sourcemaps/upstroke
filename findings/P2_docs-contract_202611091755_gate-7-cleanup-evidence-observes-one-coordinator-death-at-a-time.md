---
id: G7-ACCEPTANCE-M4-CUMULATIVE-CLEANUP
severity: P2
disposition: deferred
category: docs-contract
pr: TBD
reviewed_sha: 30e0e316acb17ed2117202f1ffe93c88e3e808ee
location: src/engine/topology/coordinator.rs:17258
provenance: undetermined
first_bad:
guard: the change that records, on Unix, the cumulative physical outcome across successive coordinator deaths, showing that what earlier deaths left is settled and does not accumulate
---

## Failure sequence

Gate 7's repeated-death requirement (M4 in its acceptance map) asks for a cumulative physical
observation. Across successive coordinator deaths, what each dead coordinator leaves behind must
be settled, and residue must not accumulate from one death to the next. On Unix that settling is
done by the surviving cleanup reapers, which hold R28 while they settle the dead coordinator's
process groups.

    a coordinator dies, its reapers settle its pipelines' processes under R28, and the next
    coordinator resumes the run
    -> the accepted evidence observes one such death: one real coordinator death with three
       simple workers, whose processes disappeared. The Unix test
       `a_killed_coordinators_reapers_settle_its_pipelines_processes_under_r28_before_the_next_one_resumes_at_width_three`
       (src/engine/topology/coordinator.rs:17258) has the same one-death shape
    -> separate single-crash point fixtures each observe one real death, its R28 settlement and
       its recovery, and are credited case by case
    -> no record accepted for this requirement observes the physical state after a second or
       later death, so nonaccumulation across successive deaths has not been observed

This is an evidence-scope gap, not a demonstrated failure. The finding does not establish a leak,
accumulation or unsettled residue, and it claims none. The existing evidence keeps its scope. That
includes the one-death, three-worker observation, the separate point fixtures (four real deaths,
each with its R28 settlement and recovery), and the source, reaper and R28 controls that support
ordinary group settlement and exclusion. On Windows, the compositional containment credit was
accepted, and it is not part of this finding. Fixtures elsewhere that model repeated incarnations,
such as the container census's repeated-crash cases, keep whatever scope their own records give
them. This finding neither relies on them nor reassesses them. It is filed as `docs-contract`
because its deferred disposition limits what may be claimed. Until it is taken up, Gate 7's pass
must not be cited as establishing physical nonaccumulation across successive coordinator deaths.

**Why this is a deferred P2.** On 2026-11-09 the owner accepted Gate 7 as passing with this gap,
as an acceptance exception for the gate's verdict, and directed that it be recorded as a P2
finding. That exception is the only thing that makes it nonblocking. The underlying obligation
stays open and is deferred here. The exception is not new evidence, and the requirement's text is
unchanged. The location names a test whose one-death scope the missing
observation would extend. It is not a reproduction of a defect in current code, and no test is
added by this filing.

## What the change that takes this up should do

Record, on Unix, an accepted observation of the cumulative physical outcome across successive
coordinator deaths. Once each death has been settled, nothing an earlier death left behind should
remain, and nothing should accumulate across the deaths.

The requirement is the cumulative observation. This finding does not add a fixed number of
cycles, a single-run constraint, coverage of every role, an exhaustive orphan census or one
joined test. How the observation is made is for the change that takes it up.

Evidence limits. Single-death records and per-case point fixtures cannot be joined after the fact
into a successive-death sequence. Source review and reaper controls are not a substitute for the
physical observation. A defect found while closing this is filed as its own finding.
