---
id: PR435-CHECK-TO-CREATE-INTERVAL
severity: P3
disposition: deferred
category: security-trust
pr: 435
reviewed_sha: 92627ca66b6cd40db004c949b6d18bd4ed2c7010
location: src/decide.rs:1620
provenance: introduced_by_feature
first_bad: introduced with `upstroke decide` on this branch (PR #435); every shape of its claim has checked a path and then created through the same path, the checkout's `.upstroke/proposals/<run-id>/` chain before the claim moved into the run's private directory, and that directory since
guard: the next change to how `upstroke decide` checks and creates its claim (`Claim::take`, `Claim::admits`, `Claim::write` in src/decide.rs), or to `rundir::create_private_dir`
---

## Failure sequence

Left open by the narrowing of `upstroke decide` under the owner's decision of 2026-11-27 on item
52, which asked the design either to drop its statement of a check-to-create window or to state
exactly what remains. A window remains, and design §15 ("What the checks leave") now states it.
Read from the code at the reviewed head; nothing was run.

    decide admits the run; its private directory P is a real directory, the user's, mode 0700
    -> Claim::take checks P again: closed_private_dir(P), a symlink_metadata read of the path
    -> a process that can write P's parent renames P away and puts a link to a directory D at P
    -> std::fs::create_dir(P/decide) resolves P again, through the link, and creates D/decide
    -> util::fsync_dir(P) syncs D; the settings file's check (Claim::admits) then reads P,
       sees the link and refuses, so no agent is asked
    -> D holds an empty `decide/` it did not hold, and the run's own private directory holds no
       claim: once P is put back, a later decide of the run is not refused as claimed

The same interval stands between `Claim::admits` and the `std::fs::File::create_new` that
`write_new` makes of each of the claim's three files: a link put at `P/decide` in it has the
file created where the link points. On Unix only the user decide runs as, or root, can rename
anything in a private directory the check admits (the user's own, closed to group and other); its
parent is not checked, and whoever can write that parent can rename P itself. On Windows no owner
and no access list is checked at all.

At the reviewed head, `src/decide.rs:1619`-`:1621`, and after this run's comment removal at
`:1426`-`:1428`:

```rust
    fn take(&self, run_id: &str) -> Result<(), UpstrokeError> {
        closed_private_dir(&self.private).map_err(|reason| untrusted_private(run_id, &reason))?;
        match std::fs::create_dir(&self.dir) {
```

and `Claim::write`, which checks and then creates by path:

```rust
    fn write(&self, name: &str, bytes: &[u8]) -> Result<(), UpstrokeError> {
        let path = self.file(name);
        self.admits()
            .map_err(|reason| refused(format!("`{}` was not written: {reason}", path.display())))?;
        write_new(&path, bytes)
    }
```

What the checks do keep: no call, read or other write runs between a check and its create, and
the raw output's and the proposal's checks are made once the reviewer's call has returned, so a
change made while the call runs is seen
(`decide::tests::a_claim_changed_while_the_reviewer_runs_is_not_written_into`). No test changes
the private directory or the claim inside the interval itself.

## What the change that takes this up should do

- Hold one handle from the check to the create: open the private directory once without
  following a link, check its owner and mode on that handle, and create the claim and each of
  its files relative to it, never by a path resolved a second time; or check the private
  directory's parent as well, so that only the user decide runs as can rename what the check
  admitted, and say which of the two the design relies on.
- Keep every refusal before the claim one that writes nothing and asks no agent, and keep the
  check before each file once the call has returned.
- Make it observable with a test that swaps the private directory, and then the claim, for a link
  between the check and the create, through a hook the test controls, asserting that nothing is
  created where the link points, beside the undisturbed run, which is claimed and asked.
