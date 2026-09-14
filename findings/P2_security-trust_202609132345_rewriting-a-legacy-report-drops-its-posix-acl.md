---
id: PR10-LEGACY-REPORT-REWRITE-DROPS-ITS-POSIX-ACL
severity: P2
disposition: deferred
category: security-trust
pr: 279
reviewed_sha: bab5a4b7067f4d7a6b7e9a7a59f5cc3aa7517ccb
location: src/rundir.rs:596
provenance: introduced_by_feature
first_bad: 763c5c9b
guard: the rustdoc of `rundir::Preserved` and `rundir::write_report` state that a POSIX ACL is not carried; the reproduction recipe `rewriting_report_keeps_its_posix_acl` (below) is unexecuted; the change that takes this up carries the ACL across the staged publication or refuses to rewrite an ACL'd report
---

## Failure sequence

Reasoned by the round-8 regression lens of PR #279; no native reproduction was executed.

A schema-3 run's `report.json` belongs to group `staff` and carries the POSIX ACL
`u::rw-,u:65534:r--,g::---,m::r--,o::---` under traversable parents with no default ACL. Its
mode reads `0640`, although ordinary members of `staff` cannot read it: with an ACL present the
group-position bits of the mode are the ACL **mask**, not the owning group's rights (acl(5)).
The run resumes and reaches `drain_and_report`, which publishes the report through
`rundir::write_report`; since PR #279's round 3 (`763c5c9b`) that is a staged, synced, renamed
publication onto a fresh inode, and since round 7 the staged file is given the existing report's
mode and group (`rundir::Preserved`, `keep_group`) — but no ACL. Applying `0640` to the new inode
grants every member of `staff` read access the ACL denied, and the named user `65534` the ACL
admitted loses it. The `fs::write` the merge base used truncated in place and kept the inode, the
ACL with it.

Reachability: a schema-3 run directory an operator ACL'd, resumed by this tree. Nothing in the
engine writes an ACL; the state is an operator's.

## What the change that takes this up should do

Execute the recipe first: `rewriting_report_keeps_its_posix_acl` beside
`rewriting_report_preserves_its_group` in `src/rundir/tests.rs` — set the ACL above with
`setfacl`, capture `getfacl -cpn`, call `write_report`, and assert the ACL after equals the ACL
before (`upstroke-build cargo test --lib rundir::tests::rewriting_report_keeps_its_posix_acl -- --exact`);
the assertion is expected to fail at `bab5a4b7` and to pass with the merge-base writer, and the
test must skip where the filesystem carries no ACL (`ENOTSUP`) or `setfacl` is absent. Then carry
the ACL across the staged publication (`acl_get_fd`/`acl_set_fd` on the staged file before its
first byte, the way the mode and group are), or refuse to rewrite a report that carries an ACL
and say so; either way the report's claim of legacy permission equivalence
(`rundir::write_report`'s rustdoc, PR #279's body) is narrowed until the ACL is carried.
