---
id: PR329-A-GENUINE-CHECKOUT-FAILURE-AFTER-THE-TAKEOVER-REFUSES
severity: P2
disposition: deferred
category: liveness
pr: 329
reviewed_sha: 4a126215be58fea36271db3423180d38c1bf3183
location: src/workspace_manager.rs:2649
provenance: introduced_by_feature
first_bad: PR #329's design round 8 (its record §7.3, R14), which answers an add failure after Git may have taken the destination over with a refusal; prior ID FUB-D5-GENUINE (PR #329's design review round 5, at the reviewed SHA above), whose consequence this is, reopened deliberately
guard: the PR11 orchestrator's one consolidated owner question after design reviews B8 (#329), C4 (#330) and D3 (#331), where round 8's narrowing is classified; it becomes accepted-risk if the owner keeps PR #329's record §7.3, and otherwise the closure needs machinery that tells a genuine checkout failure from a prune's deletion
---

## Failure sequence

    a candidate's verification snapshot cannot be checked out at its destination (a path this platform
    refuses, such as Windows' limits without core.longpaths or macOS's 1,024-byte paths; a required filter
    that fails; an object a partial clone has not fetched; a destination the filesystem will not write)
    -> the snapshot add fails after Git took its destination over, and Git's junk removal removes it
    -> PR #329's access (round 8) cannot tell this end state from a prune that deleted the add's
       registration, and refuses at once as `RegistryRefused`
    -> `run::verified` passes the refusal on (`src/engine/topology/run.rs:289`); the command ends resumably,
       and the verification is settled interrupted
    -> every resume re-verifies the candidate, meets the same failure and refuses again
    -> where master returns Git state and `not_repairs` defers the candidate, then parks it with a question
       at `max_defers`, the run instead stops at it on every resume until the content or the environment
       changes; no deferral is spent and nothing durable is recorded

## Why it is accepted for now, and what closing it would take

**The trade** is PR #329's record §7.3, option (ii). After a failed add, a destination that is absent, or an empty
directory the access cannot remove, is the same end state whether Git's checkout failed or a prune deleted the add's
registration (FUB-D6-PRUNE). PR #329 had two answers available.
- **(i) Return it as Git state.** A prune's deletion then spends a valid candidate's deferral or parks it (executed:
  Git after one attempt in every prune interleaving on 2.43.0, 2.50.1 and 2.55.0).
- **(ii) Refuse it.** That is round 8's choice. This file is its cost.

Round 7's attempt to tell the two apart, a registry-free checkout probe, carried two executed P1s and is withdrawn.

**What the refusal costs** (`~/orch-pr11/logs/pr11_fub_design8/witness/FIGURES.txt`):
- one attempt, a few milliseconds plus the checkout's own runtime, and the refusal names Git's message;
- for environmental causes (a filter, a missing object, a full or read-only filesystem), the resume succeeds once the
  environment is repaired, and no deferral is spent on it;
- for a path the snapshot's destination makes too long, changing the environment answers it.

**Grading.** P2, as design review round 5 graded FUB-D5-GENUINE: no durable wrong outcome, a liveness cost on a rare
path. Its reproduction, FUB-D5-GENUINE's tree, now ends in one attempt as a resumable refusal. That is why the owner
classifies it, with round 8's narrowing, rather than the author.

**G6.** It does not block G6. No wrong outcome is recorded under Q6. The run stops resumably and names the cause.

**To close it,** something must tell a checkout that cannot be made from a deleted registration without reading the
registry. Round 7's probe did that with a second checkout, and failed (FUB-D7-SPLITINDEX, FUB-D7-CONFIG). Any such
closure is new machinery, and it is the owner's to ask for.
