---
id: G7-M2-EVIDENCE-1-WINDOWS-OBJECT-MOUNT-CORRESPONDENCE
severity: P2
disposition: deferred
category: docs-contract
pr: TBD
reviewed_sha: 7e80d921bbe98f5babd773bfb7d5236f685e770d
location: src/engine/tests.rs:19424
provenance: introduced_by_feature
first_bad: 7f70fde459b7a37df2b439959f18b3b5b7d054c8
guard: the change that adds an in-container read through `/upstroke/gitobjects` with a host-bound expected value, or that narrows the Windows correspondence wording to what the record exercises
---

## Failure sequence

On Windows the vendor runtime translates `C:/...` bind sources, so `container_acceptance_container`
skips its literal `held["Source"] == mount["source"]` assertion there (src/engine/tests.rs:19424).
`windows-container-acceptance.ps1` says correspondence is proved by behaviour (its header, line 9).
For most binds the behaviour does prove it:

- the worker's edit reaches the host checkout and is merged;
- the read-only file is read;
- the configured directory receives one line per worker and gate container;
- the view's `HEAD` is read and resolves to a commit of the coordinator's repository;
- a reviewer's writes are refused.

The Git object-store bind is the exception:

    every worker, gate and reviewer container receives /upstroke/gitobjects
    -> the worker prints the literal text of the view's objects/info/alternates
       ("/upstroke/gitobjects") and the view's HEAD, and never opens an object through the bind
    -> the record's `git cat-file -t <head>` runs in the coordinator's repository, on the host
    -> a wrong but existing, readable directory at /upstroke/gitobjects changes no assertion, so a
       passing Windows record does not show that this translated bind reaches the intended
       object store

This is an evidence-scope gap, not a demonstrated failure. No wrong mount, native failure or
ST-16/ST-20 defect was observed, and on Linux and macOS the literal source equality is asserted.
The evidence review of 7e80d921 classified it `evidence_scope_gap`, which is outside this ledger's
vocabulary. It is filed as `docs-contract` because its deferred disposition limits what may be
claimed. Until this is taken up, a native Windows pass must not be cited as establishing
in-container access to the object store through the translated bind. The record's other mount,
view-metadata and inspect facts keep their scope.

## What the change that takes this up should do

One of two things. Add one ordinary in-container read through the object-store target, with an
expected value the host binds independently: for example, the worker prints a digest of the
bytes of its view `HEAD`'s object file read under `/upstroke/gitobjects`, compared with the same
file read on the host. Or narrow the recipe's and the record's correspondence wording to the
behaviour actually exercised. Neither is a Git-feature campaign.
