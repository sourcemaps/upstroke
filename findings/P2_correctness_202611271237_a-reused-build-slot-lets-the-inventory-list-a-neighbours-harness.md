---
id: PR439-INVENTORY-REUSES-PREVIOUS-SLOT-HARNESS
severity: P2
disposition: deferred
category: correctness
pr: 439
reviewed_sha: f16944ee62862a5a86835dcf73c7b8be4a7fe8b3
location: .github/scripts/test-categories.py:286
provenance: introduced_by_feature
first_bad: 0d3018d9293b58d27d376bfdd095f527cf55884c — "named-ci-category-legs", the commit that added the standalone `check` and its route through `upstroke-build`; origin/master has no category check
guard: the build box's tooling, not the check — `~/bin/upstroke-build` owning freshness when a slot changes checkout
---

## Failure sequence

**The wrapper is `~/bin/upstroke-build`, which is not in this repository**; the check that
trusts it is. `PR262-WARM-BUILD-SLOT-SERVES-ANOTHER-WORKTREE` is the open row whose subject is
the wrapper itself.

    checkout A runs Cargo through `upstroke-build` and leaves its test harnesses in the slot
    it took
    -> `upstroke-build` takes the first slot whose lock is free and binds `CARGO_TARGET_DIR`
       to it (~/bin/upstroke-build:151-177); it never reads the directory it was called from,
       so checkout B can take the slot A has just left
    -> B runs the standalone check: `cargo()` (.github/scripts/test-categories.py:190-193)
       sends each Cargo call through the wrapper when `upstroke-build` is on PATH, and the
       check asks Cargo for the all-target, all-feature listing (lines 286-287), then for the
       listing with every category filter skipped (lines 290-291)
    -> Cargo reports A's harness at that slot's path fresh for B's tree, does not relink it,
       and runs it with `--list`
    -> the check validates and counts A's registrations: one B added and left uncategorised
       is never listed, and the check passes

At f16944ee the check held one slot from its build through every listing
(`.github/scripts/test-categories.py:425` there; 35124a0b, `PR439-INVENTORY-OUTLIVES-BUILD-SLOT`)
and listed the executables its `cargo test --no-run` reported (`build()`, line 274 there). That
closes the window after the slot is released. It does not change what Cargo decides inside the
slot: a harness Cargo reports fresh is listed whichever checkout linked it. The narrowing
(feefa974) returns the check to listing through Cargo, so each of its two Cargo calls takes a
slot of its own.

`PR262-WARM-BUILD-SLOT-SERVES-ANOTHER-WORKTREE` records, measured on the build box, that the
wrapper chooses a slot by lock and that slots are shared between worktrees, and records a
160-test difference on byte-identical source that it leaves unexplained.

Established by reading the check at f16944ee and at this task's head, and the wrapper; nothing
was run here. A slot in which Cargo reports another checkout's harness fresh was not reproduced
here.

## Why it is deferred

- The remedy ruling r85 names is the build wrapper's, not the check's. The wrapper is outside
  this repository, so no change to the check closes this row, on the same terms as
  `PR262-WARM-BUILD-SLOT-SERVES-ANOTHER-WORKTREE`.
- Ruling r85 narrows #439 and adds no new script logic; the slot handling the check carried at
  f16944ee is what the narrowing removes.
- Unwitnessed: there is no failing test, reproduction or mutation witness.
- CI is outside it: the check runs Cargo directly where no `upstroke-build` is on PATH
  (`.github/scripts/test-categories.py:191`).

## What the change that takes this up should do

- Make the wrapper own freshness when a slot changes checkout. It knows the slot it takes and
  the directory it was called from, so it can record each slot's last checkout and, when the
  next run comes from another, make that run's build its own or refuse it. Whether that is a
  clean, a refusal, or slots bound to checkouts for gate runs is a cost decision for the
  wrapper (`PR262-WARM-BUILD-SLOT-SERVES-ANOTHER-WORKTREE`, its first point).
- Leave the check listing through Cargo, as it does at this task's head, with no slot or
  freshness logic of its own.
- Witness it on the build box with one slot under a scratch base (`UPSTROKE_SLOTS=1` and
  `UPSTROKE_RAMTARGET`, ~/bin/upstroke-build:48-49): build checkout A, then from checkout B,
  which adds one uncategorised registration, run the check through the wrapper. The check must
  name B's registration, or the wrapper must refuse.
