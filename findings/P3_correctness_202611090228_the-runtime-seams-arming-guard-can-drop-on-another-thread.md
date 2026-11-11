---
id: G7-M2-REGRESSION-001-TLS-GUARD-SEND
severity: P3
disposition: deferred
category: correctness
pr: TBD
reviewed_sha: 7e80d921bbe98f5babd773bfb7d5236f685e770d
location: src/engine/topology/entry.rs:1631
provenance: introduced_by_feature
first_bad: 7f70fde459b7a37df2b439959f18b3b5b7d054c8
guard: the next change to the `#[cfg(test)]` half of `engine::topology::entry::container_runtime`
---

## Failure sequence

`container_runtime::arm` puts the journal in the arming thread's `ARMED` slot and returns
`Armed(())`. `Armed` holds only `()`, so it is `Send`, and its `Drop` clears the slot of
whichever thread it runs on.

    a test arms the seam on thread A and moves the returned guard into thread B
    -> the guard drops on B, clearing B's slot and any arming B had of its own
    -> A's slot still holds the journal after the guard is gone
    -> later runtime constructions on A keep entering the old journal, against the
       guard-scoped isolation the seam's notes state

The trigger is test-only and hypothetical. The container acceptance record and the seam's unit
test keep every guard on the thread that armed it, and no production build contains the module.
Found by source inspection in the regression review of 7e80d921; no reproducer was run.

## What the change that takes this up should do

Make `Armed` non-`Send`, for example with a `PhantomData<*const ()>` field, so its destructor can
run only on the arming thread, or bind its cleanup to the slot it armed. Keep the current
lexical use at both call sites.
