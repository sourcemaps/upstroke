---
id: PR321-ALLOWLIST-CLAUSES-DESCRIBE-THE-OLD-SCRATCH-SHAPE
severity: P3
disposition: deferred
category: docs-contract
pr: 321
reviewed_sha: f3e7003975c85d4a95e3f6b8e12050fec4f174d0
location: effects/allowlist.toml:704
provenance: introduced_by_feature
first_bad: PR64-CLEANUP-003-SCRATCH-PRECLEAN
guard: the project owner — `effects/allowlist.toml` is an instrument and a change to it takes the merge out of the standing delegation
---

## Failure sequence

`effects/allowlist.toml`'s `review` clauses describe, in the present tense, the fixture shape the
pull request that files this row replaced. A reader trusting those clauses is told the tree still
pre-cleans a predictable root. The clauses are prose: `effects::tests::
every_allowlist_entry_carries_its_justification_and_names_a_real_file` asserts only
`!entry.review.trim().is_empty()`, so **no gate reads them** and nothing failed. The claim is
stale, not broken — which is the shape that survives a green CI run indefinitely.

Measured at this head by extracting each `review` block and matching its sentences:

- **`src/rundir/tests.rs`** — "The filesystem effects are under a root from this file's own
  `scratch`, which is `std::env::temp_dir()` joined with a tag and the process id, and which opens
  with `let _ = fs::remove_dir_all(&dir)` on that predictable path". That helper now calls
  `rundir::scratch_tree::acquire`, takes a ULID-named root and removes nothing. The clause's
  per-method census also moved: `fs::remove_dir_all` 3 and `fs::create_dir` 1 are no longer what
  a denial of the three lints would count in this file.
- **`src/workspace_manager/fixture.rs`** — "`scratch` … is `std::env::temp_dir()` joined with a
  name carrying the tag, the process id and a per-call ordinal, and `Fixture::drop` removes its own
  root", and "Two effects are NOT confined to that root … `scratch` opens with
  `let _ = fs::remove_dir_all(&dir)` on that predictable path". The ordinal and the pre-clean are
  gone; `Fixture::drop` now removes only an **adopted** root, and the acquired one is its guard's.
- **`src/workspace_manager/tests.rs`** — "under roots `fixture::scratch` minted, with the
  predictable-path pre-clean that row describes".
- **`src/agent/proc/tests.rs`** — "(2) `spawn_signal_helper` and the reaper suite's `scratch`
  helper call `std::fs::remove_dir_all` on their scratch path BEFORE `create_dir_all`". Both now
  acquire. The file's `std::fs::remove_dir_all` count moves from 12 to 10.
- **`src/runner/container/{tests.rs,resolve/tests.rs,exec/tests.rs}` and `src/interaction.rs`** —
  the prose about "a scratch directory of its own making" stays true; only the per-method counts in
  their "WHAT IT NEEDS EACH FOR" censuses move.

`src/runner/host/tests.rs`'s clause is untouched and still accurate: that file was not changed.

## What the change that takes this up should do

Rewrite those clauses to describe the token-carried shape, and re-measure each census by denying
the three governed lints in the file and counting the errors, the way each clause says it was
measured. It is a small edit and it is deliberately not in the pull request that files this row:
`effects/allowlist.toml` is one of the instruments `MAINTAINING.md` step 7's first limb names, so a
diff carrying it cannot merge under the standing delegation. `PREDICTABLE-SCRATCH-ROOTS-BEYOND-VALIDATE`
predicted exactly this — "Repairing that helper — three lines — falsifies both, so the repair
reaches an effect allowlist and stops being a delegated merge" — and this row is that prediction
coming due.

**Severity.** `P3` is this row's own judgement: the stale text is a governance record a reviewer
reads, not a rule a build enforces, and the direction of the error is safe — it claims a hazard the
tree no longer has rather than hiding one it does.
