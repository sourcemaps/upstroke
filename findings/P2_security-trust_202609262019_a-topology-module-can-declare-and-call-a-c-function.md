---
id: PR325-A-TOPOLOGY-MODULE-CAN-DECLARE-AND-CALL-A-C-FUNCTION
severity: P2
disposition: deferred
category: security-trust
pr: 325
reviewed_sha: c0aa018ce6179eed86e30bfc3e52cc08fdb88fe6
location: clippy.toml:113
provenance: pre_existing
first_bad:
guard: project owner — the `unsafe_code` governance `PR7-WRAPPERS-EMPTY-DOMAIN` names as the direction for its linkage route, not adopted by #325
---

## Failure sequence

`decisions.effect_site_inventory.mechanism` (3) holds that a topology module reaches no effect except
through a funnel that takes a site, because every effectful primitive is denied **by path**: 32 of
`clippy.toml`'s entries name a C-library function under `libc::` (`libc::unlink` at line 113, with
`libc::write`, `libc::open`, `libc::kill`, `libc::rmdir`, `libc::execv` and the rest), and **in a
topology module that forbids all three governed lints**, a call to a denied path is an error no
attribute can lower. Not every topology module does: the funnels among them —
`src/runner/host.rs`, `src/runner/container.rs`, `src/workspace_manager.rs` and others — allow some. Nothing denies the C function itself. So, reasoned from the code:

1. A topology module that forbids `clippy::disallowed_methods`, `clippy::disallowed_types` and
   `clippy::disallowed_macros` declares the function in its own foreign block --
   `unsafe extern "C" { safe fn unlink(path: *const std::ffi::c_char) -> i32; }` -- and calls it,
   `unlink(path.as_ptr())`. A `safe fn` in an `unsafe extern` block is called without an `unsafe`
   block; Rust 2024, which this crate is, has both.
2. Clippy's disallowed-methods lint matches the callee's path. The callee is the crate's own foreign
   item, `upstroke::topology::..::unlink`, not `libc::unlink`, so no entry matches it and the
   module's `forbid` has nothing to refuse.
3. The census that ties `libc` to the denylist, `effects::tests::classification::checks::libc_items_are_classified_and_denied`,
   collects the items the source names after the literal text `libc::`. The declaration names no
   `libc::` item, so it asks nothing of it.
4. `unsafe_code` is allowed by default and appears nowhere in the tree: nothing refuses the
   `unsafe extern` block. No census reads a foreign block, `#[unsafe(no_mangle)]` or
   `#[unsafe(export_name)]` in a topology module. The classification census does not reach it:
   **it reads only a classified module's reachable names, and a privately declared foreign item is
   none.** (Some topology modules *are* classified — `src/workspace_manager.rs` with its children and
   `src/runner/host.rs` are classified by `effects/wrappers.toml` — so the earlier wording "a topology
   module is not classified" was too broad; what matters is that the census reads reachable names.)
   Its reader would not see the declaration anyway:
   `effects::declares_visibility` strips `extern`, `unsafe`, `const` and `async` before a `fn` but not
   `safe`, so a `pub safe fn` in a foreign block reads as private.
5. The topology module unlinks a file with fmt, clippy under `-D warnings`, and every census green.

The same holds of every function `clippy.toml` denies under `libc::`, and of any C function the
denylist never named.

**Reasoned, not executed.** The code above is read, not exercised: no foreign block was written into
a topology module and no clippy run was made against one. A review of #325 found the route by reading
at `c0aa018c` and flagged it rather than filing it; #325's fourth round files it on the orchestrator's
instruction.

## Reachability, against the owner's rule of 2026-09-11

*A P1 blocks a merge only if it can happen in normal use, or someone without push access can trigger
it.* **Neither limb holds.** Not normal use: it is a change to the source, not a run of the product.
Not someone without push access: the declaration reaches the tree only through a merge, which is the
owner's act or a delegate's, and it is written in the topology module's own text -- a foreign block
and a call -- in the diff the reviewer reads. A contributor without push access can propose it and CI
stays green, which is why it is filed rather than left in a review document.

**Why P2 and not P1**: the consequence is `PR7-WRAPPERS-EMPTY-DOMAIN`'s -- an effect a topology module
reaches with enforcement green -- but that finding's routes hide what compiles from the text a reviewer
reads (a name spelt by expansion, a module an alias includes, an item reached by a symbol a macro
writes), and this one does not. It is its own finding because it goes around the denylist rather than
through a legacy wrapper: it needs no allowing module, no classified function and no macro, so it is a
different mechanism from that P1's, and a repair of either leaves the other. The owner may reclassify.

## What the change that takes this up should do

Owner, as the ledger records it: project owner.

- **The direction, not adopted here**: `#![forbid(unsafe_code)]` in every topology module. It refuses
  an `unsafe extern` block, and a `#[unsafe(no_mangle)]` or `#[unsafe(export_name)]` definition, on
  rustc 1.97.1 and 1.85.0 (measured on scratch files only, for `PR7-WRAPPERS-EMPTY-DOMAIN`'s linkage
  route: `~/findings-sweep/orch-p1/p1-six-modules-r3/unsafe-code-probe/`). It cannot be one line at a
  crate root, because the process funnels use `unsafe`; which modules can forbid it, and a pin that
  holds them to it, are not measured against this tree.
- Short of that, a census that refuses a foreign block, a symbol-export attribute and a `safe` item
  outside the funnels, over `production_code`, with `declares_visibility` reading `safe` as the
  qualifier it is -- a recogniser, with that class's limits, which is why the lint is the direction.

## Provenance

Found by reading at `c0aa018c` by a review of #325 (the four-question lens), which reported it and the
`safe` omission in `declares_visibility` without filing either. Filed by #325's fourth round, which
changes nothing it depends on.
