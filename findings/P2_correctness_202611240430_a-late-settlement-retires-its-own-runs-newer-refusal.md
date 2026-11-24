---
id: PR409-LATE-SETTLEMENT-RETIRES-OWN-RUNS-NEWER-REFUSAL
severity: P2
disposition: deferred
category: correctness
pr: 409
reviewed_sha: 759cbcc38bf599e42c0c838411829a9d9322692c
location: src/engine/topology/run.rs:2855
provenance: pre_existing
first_bad:
guard: the first change to how a run retires a held pool
---

# A late settlement retires its own run's newer refusal

Found while ruling r50 settled #409's round three (`PR409-DELAYED-SETTLEMENT-CLEARS-NEWER-LIMIT`), and filed deferred under that ruling. It was read in the code, not run.

## Failure sequence

    inside one run, an attempt's worker serves pool `p`, and the attempt then runs its gates for minutes
    -> meanwhile another attempt of the same run is refused on `p`: the run writes `pool_exhausted`, naming a reset, and holds `p` until that reset
    -> the first attempt is promoted, and `record_promoted` (`src/engine/topology/run.rs:2852-2856` at the reviewed commit) calls `retire_served_pools`
    -> `retire_served_pools` removes `p` from the run's exhausted pools, although nothing served `p` after the refusal
    -> the run may bind `p` again before its reset, and so may a resume, which replays `exhausted_pools_of` and removes `p` at the same point

## What is established

- This is `master`'s rule for one run. `master`'s replay retires every pool a settled record served, at
  `candidate_prepared` as at `attempt_finished` (`4c9c566b:src/capacity.rs:269-289`), and the live run
  mirrors it.
- #409's cross-run fold no longer applies it across runs: there, service is dated by its attempt's start
  plus its duration, and a success retires another run's refusal only when that service is known and
  later than the refusal. Inside one run the fold keeps `master`'s rule, so that a resume replays exactly
  what the live run applied.
- #409 excludes the engine: on its branch `src/engine/topology/run.rs` is `master`'s, unchanged.
- A failed attempt's settlement retires the same way (`settle_failure`, `src/engine/topology/run.rs:2765`
  at the reviewed commit, replayed at `attempt_finished`), so a worker that served `p` before the refusal
  and then failed a gate clears it too.

## Remedy

- **Date a promotion's service in the live run as well**, by its attempt's start plus its duration, and
  retire a held pool only for a refusal older than that service. The live run holds each pool with its
  reset alone (`exhausted_pools`, `src/engine/topology/run.rs:1435`), so it has to keep the refusal's
  time too.
- **Change the replay (`exhausted_pools_of`) and the live run together**, so that a resume still applies
  exactly what the live run did. The failed attempt's settlement above changes with them.
- Its repair branch: `fix-P2/correctness_a-late-settlement-retires-its-own-runs-newer-refusal`.
