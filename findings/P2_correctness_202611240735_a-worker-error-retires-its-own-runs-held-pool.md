---
id: PR409-WORKER-ERROR-RETIRES-OWN-RUNS-HELD-POOL
severity: P2
disposition: deferred
category: correctness
pr: 409
reviewed_sha: 79682552d7091a3366da0cad3a03dc7fa25c0811
location: src/engine/topology/run.rs:2765
provenance: pre_existing
first_bad:
guard: the first change to how a run retires a held pool
---

# A worker error retires its own run's held pool

Found while ruling r51 settled #409's round four (`PR409-WORKER-ERROR-CLEARS-OTHER-RUNS-LIMIT`), and filed deferred under that ruling. It was read in the code, not run.

## Failure sequence

    inside one run, an attempt is refused on pool `p`: the run writes `pool_exhausted`, naming a reset, and holds `p` until that reset
    -> another of the run's attempts on `p`, in flight when the refusal came, fails with no answer from the provider: a worker `AgentError` such as Claude Code's `Could not connect to api.anthropic.com: Network is unreachable`, with no usage and no cost
    -> `settle_failure` calls `retire_served_pools` (`src/engine/topology/run.rs:2765` at the reviewed commit), and `served_pools` counts that failure as service, so `p` is removed from the run's exhausted pools, although `p` served nothing
    -> `another_pool_can_serve` (`src/engine/topology/run.rs:2797-2816`) may then rerun a refused task on `p` at once, before its reset: a worker of `p`'s agent refused on another of its pools fails over, and the rerun may bind `p`, which the run no longer holds
    -> a resume may do the same: it replays `exhausted_pools_of`, which removes `p` at the same point

## What is established

- The predicate is `master`'s. `master`'s `retire_served_pools` counts every failure but an interruption
  and the worker's own rate limit as service (`48f2af7a:src/capacity.rs:321-331`), a worker that never
  reached its provider included; on #409's branch it is `served_pools`, which `retire_served_pools` calls.
- The live run binds on it. When it settles a failed attempt it retires every pool `served_pools` names
  (`src/engine/topology/run.rs:2765`), and its failover reads the pools that leaves held
  (`another_pool_can_serve`, `src/engine/topology/run.rs:2797-2816`).
- #409 carried the predicate across runs, which was `PR409-WORKER-ERROR-CLEARS-OTHER-RUNS-LIMIT`: a worker
  that never reached its provider cleared another run's refusal. Under r51 a failed record no longer
  retires another run's refusal, since it carries no `served_at`, whatever its failure, usage or cost.
  Inside one run it still does, so that a resume replays exactly what the live run applied.
- #409 excludes the engine: on its branch `src/engine/topology/run.rs` is `master`'s, unchanged.

## Remedy

- **Retire a held pool only on an attempt that passed**, a record with no failure, as #409's cross-run
  fold now does for another run's refusal. A failed attempt, a worker that never reached its provider
  included, then leaves its own run's refusals standing.
- **Change the live run and the replay (`exhausted_pools_of`) together**, so that a resume still applies
  exactly what the live run did.
- Its repair branch: `fix-P2/correctness_a-worker-error-retires-its-own-runs-held-pool`. Its guard is the
  first in-run twin's, `PR409-LATE-SETTLEMENT-RETIRES-OWN-RUNS-NEWER-REFUSAL`, whose remedy changes this
  same settlement.
