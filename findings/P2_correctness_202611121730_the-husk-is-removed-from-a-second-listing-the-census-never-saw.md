---
id: PR369-R2-HUSK-SECOND-LISTING
severity: P2
disposition: deferred
category: correctness
pr: 369
reviewed_sha: 83161210a3cb3a154625517264861a472ac3906a
location: src/rundir.rs:1703
provenance: pre_existing
first_bad:
guard: the change that makes the removal of a bare husk depend on the census's own observation, for instance by removing a planned-bare directory with `remove_dir` alone, which fails if anything is in it, with a witness of a directory that gains content between the census and the removal
---

# The husk is removed from a second listing the census never saw

## Failure sequence

The startup census plans each public directory's fate from one listing, the one taken by the
ownership proof (`unbound_shape` in `src/rundir/ownership.rs`). `remove_public_husk` in
`src/rundir.rs` (near line 1703) then removes whatever a second listing, taken at removal time,
finds. Every scan finishes before any removal.

`read_dir_names` treats `NotFound` on opening the directory as an empty directory. So:

1. A public directory is there, but opening it at planning time reports `NotFound`.
2. The ownership proof reads that as empty, and the plan is `NothingBound(Bare)`.
3. By removal time the directory is listable again. The second listing finds what is in it, and
   the removal deletes it, `events.jsonl` included.

That is `SWEEP-CLASSIFY-009`'s own failure, reached through `NotFound` rather than another error.

How a directory that is there could read `NotFound` transiently is not established. These are
candidates, from reading rather than observation:

- On Windows, the standard library maps some network errors (`ERROR_BAD_NETPATH`,
  `ERROR_BAD_NET_NAME`, `ERROR_INVALID_DRIVE`) to `NotFound`, so a share that drops out briefly
  might produce it.
- Some FUSE filesystems on Linux return a transient `ENOENT`.

Found by the frontier review of pull request #369. That pull request repairs the case of a
listing that fails with any error other than `NotFound`, and leaves this one.

## What the change that takes this up should do

- Make the removal of a planned-bare directory depend on the census's observation, not on a
  second listing: for instance, remove it with `remove_dir` alone, which fails if the directory
  has gained content since it was planned.
- Or treat `NotFound` at planning as indeterminate, not empty, where the directory was
  enumerated a moment before.
- Either way, add a witness of a directory that gains content between planning and removal.
