---
id: FUC-D2-RG
severity: P1
disposition: deferred
category: correctness
pr: 330
reviewed_sha: a9be94bc360b2e461a47c73e4d488d8a57599a15
location: reviews/2026-10-02-pr11-follow-up-c-record.md:1202
provenance: pre_existing
first_bad: predates PR11: the frozen legacy engine's Git children have never disabled automatic maintenance; graded P3 at a9be94bc on a false prerequisite and regraded P1 by #330's design review round 2 (the record's §3.4)
guard: follow-up D, before G6 — decision B (the owner's O8) unfreezes `src/workspace.rs` so that its one builder, `git_command`, gives every legacy Git child `-c maintenance.auto=false -c gc.auto=0 -c gc.autoDetach=false -c maintenance.autoDetach=false` (#331's record §3.8 and §2.5; #330's record §3.4); its grade is the owner's O10
---

## Failure sequence

R-G2 of PR11 follow-up C's record (`reviews/2026-10-02-pr11-follow-up-c-record.md`, §3.4, with §4.10's corrections),
reasoned by #330's design review round 2 (the design and concurrency lenses P1, the regression lens P2); none executed
the deletion of a completed registration.

1. A legacy (schema 1-3) command runs in a partial clone, and one of its Git children reads an object the clone lacks.
   Git lazily fetches it, and the fetch runs automatic maintenance. At 2.55.0 the `geometric` strategy is the default for
   unscheduled maintenance and includes the `worktree-prune` task (#331's round 2, its §2.5, witness rg2, executed the
   legacy engine's maintenance deleting a registration in an add's pre-`locked` state). The legacy builder sets no
   maintenance switch, and the module is PR5-frozen.
2. A topology command of the same repository adds a slot. `git worktree add` makes the registration's directory before
   it writes `locked` and `gitdir` (`builtin/worktree.c:458-494` at 2.43.0).
3. The prune reads the new entry in that window, decides "gitdir file does not exist", which is prunable with no expiry
   at all (`worktree.c:734-737` at 2.43.0), and is descheduled before it deletes.
4. The add completes and returns `Ok`; the prune resumes and deletes the completed registration by name, under a slot in
   use.

## What PR11 follow-up C's implementation changes, and what it leaves (2026-10-03)

- **R-G1, the name-reuse variant, is closed** by follow-up C's per-incarnation slot instances: a prune paused after
  deciding on an old entry deletes that old instance's name, never a replacement's, because no incarnation recreates a
  name another created (the record's §3.3.3; the implementation's DESC witnesses through the production funnels).
- **R-G2, this file, is not**: the prune reads the current instance's own new entry. Neither U nor Q reaches a process
  the topology engine did not start. The topology manager's own Git children no longer start maintenance (the switch
  set, `both_builders_carry_the_engine_switch_set_and_its_bindings`), so the starter is the legacy engine's.

## What the change that takes this up should do

Follow-up D's legacy change, as its guard says: the four maintenance switches on every legacy Git child, executed for
the legacy engine's lazy fetch on Git 2.43.0, 2.50.1 and 2.55.0 by #331's round 2. With it, R-G1 and R-G2 are both
closed for maintenance a legacy command starts. Maintenance a process the engine did not start runs (the user's own Git,
an IDE's) is the external-prune class, `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`.

**G6:** applicable to Q1, ST-18 and INV-22; P1 blocks G6 until D lands or the owner reclassifies (O10) or excludes it.

## Filed

At PR11 follow-up C's implementation, on the orchestrator's brief (`~/orch-pr11/briefs/pr11_fuc_impl.md`, "Findings at
this touch"): #330's ledger carried it as a `deferred` row with no file since design round 3.
