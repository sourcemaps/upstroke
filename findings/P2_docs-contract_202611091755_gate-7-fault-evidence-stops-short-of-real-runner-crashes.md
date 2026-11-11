---
id: G7-ACCEPTANCE-M3-FAULT-EVIDENCE
severity: P2
disposition: deferred
category: docs-contract
pr: TBD
reviewed_sha: 30e0e316acb17ed2117202f1ffe93c88e3e808ee
location: src/engine/topology/recover/tests.rs:7843
provenance: undetermined
first_bad:
guard: the change that records an accepted mapping from Gate 7's recovery fault inventory to real-runner outcomes, together with a durable recovery after two actual crashes of a real runner against a real repository
---

## Failure sequence

Gate 7's recovery requirement (M3 in its acceptance map) asks for two things. No record accepted
for the gate supplies either one in full.

**The inventory mapping.** The requirement's fault inventory names the publication, owner and
commit classes, the Object and Event classes, and residues sampled inside Git. Each is to be
classified and recovered on a real runner.

    a reader looks in the accepted Gate 7 records for each inventory class
    -> thirteen actual deaths in a run's creation prefix were classified, and separate deletion
       tests and finite finalization matrices each cover their own cases
    -> the recorded fault-site and result facts have not been reconciled against the inventory,
       so the accepted mapping is incomplete
    -> no accepted record shows every class classified and recovered on a real runner

**The two-crash sequence.** The nearest existing evidence is
`unsynced_merge_prepared_two_crash_barrier_before_cas_then_power_loss_keeps_log_and_ref_agreeing`
(src/engine/topology/recover/tests.rs:7843):

    the first crash is constructed: `crash_with_unsynced_merge_prepared` leaves an unsynced
    merge_prepared line and drops the incarnation
    -> the second crash is an actual abort: a child process resumes the run and is killed at the
       task_merged append
    -> the write loss is simulated: `lose_unsynced_writes` cuts the log back to its synced prefix
    -> a third resume records the completed swap without swapping again, repairs nothing in the
       creation prefix, and replays the same way twice

Evidence of this shape establishes stable-prefix ordering and the modelled sequence. Its first
crash is constructed, so it cannot be relabelled as the first crash of a real runner, and no
accepted record shows durable recovery after two actual crashes.

This is an evidence-scope gap, not a demonstrated failure. The finding does not establish
corruption, a lost commit or a failed recovery, and it claims none. It also does not say that no
fault evidence exists. The thirteen creation-prefix deaths prove their classifications, and the
deletion tests, the finalization matrices and the modelled sequence keep their own scope. Like
G7-M2-EVIDENCE-1-WINDOWS-OBJECT-MOUNT-CORRESPONDENCE, it is filed as `docs-contract` because its
deferred disposition limits what may be claimed. Until it is taken up, Gate 7's pass must not be
cited as establishing real-runner recovery across the fault inventory, or durable recovery after
two actual crashes.

**Why this is a deferred P2.** On 2026-11-09 the owner accepted Gate 7 as passing with this gap,
as an acceptance exception for the gate's verdict, and directed that it be recorded as a P2
finding. That exception is the only thing that makes it nonblocking. The underlying obligation
stays open and is deferred here. The exception is not new evidence, and the requirement's text is
unchanged. The location names the test whose scope stops short. It is not a reproduction of a
defect in current code, and no test is added by this filing.

## What the change that takes this up should do

Close both parts. Neither closes the finding alone.

- **The mapping.** An accepted record that maps every inventory class (publication, owner and
  commit; Object; Event; residues sampled inside Git) to an observed real-runner outcome, showing
  each one classified and recovered. The recorded fault-site and result facts must be reconciled
  against the inventory as part of that mapping, not dropped.
- **The sequence.** A durable recovery after two actual crashes of a real runner against a real
  repository. The recovered log and refs must agree, and replay must be stable.

Evidence limits. Existing records count toward closure only for what they observe. A constructed
or simulated crash is not an actual one, and a modelled sequence is not to be renamed a
real-runner sequence after the fact. A defect found while closing this is filed as its own finding.
