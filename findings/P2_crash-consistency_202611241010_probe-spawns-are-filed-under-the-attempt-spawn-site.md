---
id: PR4-SPAWN-SITE-PROBE-CONTEXT
severity: P2
disposition: deferred
category: crash-consistency
pr: 4
reviewed_sha: 48f2af7ad20c1db04ddd1a0e73bab09f9bd4bcfa
location: src/topology/effects/sites.rs:1413
provenance: pre_existing
first_bad:
guard: the change that next opens the `ProcessSite` inventory in `src/topology/effects/sites.rs` to give a pre-flight spawn its own site context (the record named the PR6/PR7 implementer)
---

## Failure sequence

`Process.Spawn` is one site with one adjacency (`After(AttemptStarted)`) and one fault row
(`T-ATTEMPT`), but PR4 routes five roles through it, and two of them, `Probe(Shell)` and
`Probe(Agent)`, are `RunnerPreflight`, ordered at P4, before P6's `run_started`. A crash prefix at
a probe spawn is effect-before-`run_started` (`T-RUNSTART` on a fresh run, `T-RESUME` on a resume),
while the site it is filed under says event-before-effect in `T-ATTEMPT`. ST-07 evidence over
`Process.Spawn` therefore does not cover the probe prefixes
(`48f2af7a:reviews/FINDINGS.md:128`, §2).

The record split the finding in two. The containment hooks firing on both probe paths was PR4's
own obligation and was repaired under `PR4-CONF-002` (`48f2af7a:reviews/FINDINGS.md:105`). What
stayed deferred is the **site variant**: a probe-specific semantic context, its adjacency and its
fault row. It was settled for PR4 as an accepted deviation under the owner's 2026-08-20 ruling that
the frozen effect inventory stays frozen, and the 2026-08-31 audit carries it as class V4 with the
PR6/PR7 implementer as owner (`48f2af7a:reviews/FINDINGS.md:3741`). No later record line closes it.

The record carried no severity label, so **P2** here is this filing's rating (an unrated record
finding is filed as P2), not the reviewer's word. The record names no reviewed SHA for this row,
so `reviewed_sha` is this head.

## Verified on `48f2af7a` (docs PR 5, 2026-11-24)

The gap is still there. `ProcessSite` has only `Spawn` and `Terminate`
(`src/topology/effects/sites.rs:1382`). `Spawn`'s adjacency is still
`Adjacent::After(DurableEvent::AttemptStarted)` (`src/topology/effects/sites.rs:1413`). Every
containment point, for every role, is hooked at `SPAWN_SITE`
(`src/runner/contract.rs:486`, `:521`). The census
`runner::contract::tests::the_spawn_site_files_every_role_under_one_context_and_the_count_says_which`
(`src/runner/contract.rs:1295`) still asserts the site's adjacency and fault row and that exactly
`probe(shell)` and `probe(claude-code)` spawn outside the context the site names. That test keeps
the gap from growing; it does not close it.

## What the change that takes this up should do

Give pre-flight spawns their own site context in the effect inventory: a `Process` site (or a
context on `Spawn`) whose adjacency places it before `run_started` and whose fault row is
`T-RUNSTART` / `T-RESUME`. Route the two probe roles through it in `src/runner/contract.rs`, so ST-07
evidence covers the probe crash prefixes under the row that describes them. Then change the census
test's expected outside count from 2 to 0.
