---
id: FUC-D5-ACCOUNT
severity: P2
disposition: deferred
category: docs-contract
pr: 330
reviewed_sha: 3002682363161c528946e482619d6aa77d933aca
location: reviews/2026-10-02-pr11-follow-up-c-record.md:3312
provenance: introduced_by_feature
first_bad: 3002682363161c528946e482619d6aa77d933aca (the final sweep of design round 5, which narrows FUC-D4-TERMACCOUNT's window to its own last scan and cannot close it)
guard: the owner's decision O4 (#330's D1): either the coordinated amendments A-1 to A-12 of the PR11 decision appendix's §5.2, which account a late instance resumably_open in its slot's row (an ACCEPT, and `src/engine/topology/ledger.rs`'s observe, equation, check and export with it), or §5.3's decline, under which G6 answers Q2 and "ledgers balance" only for runs no earlier incarnation's writer crosses the final sweep; before G6
---

## Failure sequence

R-UR (P3: disk, and a stray registration in the user's `git worktree list`) and its accounting consequence FUC-D5-ACCOUNT
(P2), as PR11 follow-up C's record states them (`reviews/2026-10-02-pr11-follow-up-c-record.md`, §5.4's window table,
§5.7's E-FUC-3 items 2 and 8, §5.8's R-UR row) and #330's design review round 5 graded them (the triage
`~/orch-pr11/reviews/review-330-d5-triage.md:23` and `:63`).

1. An earlier incarnation's Git writer is still running when terminal finalization runs: its add had not yet run when its
   slot was reclaimed (executed: the record's §4.3 witness, and `p3_a_late_add_released_after_the_successors_walk_is_found_and_removed`).
2. Finalization's last step, `WorkspaceManager::remove_execution_root`, sweeps every earlier incarnation's instance before
   it removes the root (`sweep_earlier_instances`; `the_final_sweep_removes_an_instance_an_earlier_incarnation_added_after_the_scrub`).
3. The writer acts after the sweep's scan:
   - before the root's emptiness check: the root is kept, finalization returns `execution_root_removed: false` with the
     instance present;
   - after the root's removal: the late add recreates its leading directories, the root included;
   - while its registration held no `gitdir` at the scan: the scan passes it over and the add completes later.
4. **The instance has no class in the run's resource accounting.** INV-22 and Q2 require exactly one class and one row
   for every owned resource in every state; `src/engine/topology/ledger.rs` requires R9, R10, R18 and R24 pruned for every
   ended outcome (`:917-928`); a late snapshot add can make an ephemeral commit reachable again, so R27's monotone
   `unreachable_objects` fails (`:901`). E-FUC-3's items 2 and 8 propose a terminal-accounting exception; the decision
   appendix's A-1 to A-12 propose an accounting of it instead. **Neither is adopted.**

## What PR11 follow-up C's implementation does about it (2026-10-03)

Nothing beyond the sweep: by the orchestrator's brief, accounting (O4) is unadopted, so `ledger.rs` and Halted are
unchanged, no late-instance exception is implemented, and no accounting or output-preservation check is weakened. A late
instance an earlier incarnation's still-running writer creates after the final sweep's scan stays unaccounted, so **G6's
ledger and Q2 clauses stay blocked for any run such a writer crosses** (the appendix's O4, "without a decision"). The
run's next terminal finalization or resume reclaims what it then finds; otherwise the operator removes it.

## What the change that takes this up should do

The owner's O4, as the guard says. No design that prevents a late instance exists in this programme (the appendix's
statement A-B, §5.3): prevention needs an observation, taken before the final sweep, that no earlier incarnation's Git
writer can still act, and U observes no process by design.

## Filed

At PR11 follow-up C's implementation, on the orchestrator's brief (`~/orch-pr11/briefs/pr11_fuc_impl.md`, "Findings at
this touch").
