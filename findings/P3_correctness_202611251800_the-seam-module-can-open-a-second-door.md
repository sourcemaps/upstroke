---
id: PR429-SEAM-MODULE-CAN-OPEN-A-SECOND-DOOR
severity: P3
disposition: deferred
category: correctness
pr: 429
reviewed_sha: 303379a2843368d9bb7003d787cc1314a4f13678
location: src/capacity/client.rs:1
provenance: introduced_by_feature
first_bad: 303379a2843368d9bb7003d787cc1314a4f13678
guard: the change that adds a second CapacityClient
---

# The capacity seam's own module can open a second door

## Failure sequence

From #429 round two's frontier pass (comment 7169622396), reasoned, not run:

    a crate-visible item added to src/capacity/client.rs builds or exposes a second client
    -> an engine file reaches it as crate::capacity::client::<item>
    -> neither the compiler nor the census refuses it
    -> that client "answers capacity questions an installed handler never sees", as the message of
       the_builtin_client_is_built_only_by_the_run in src/capacity/client.rs puts it

What the compiler holds after this repair:

- `src/capacity.rs` declares the module `pub(crate) mod client;`, so it is crate-private.
- In `src/capacity/client.rs`, `struct BuiltinClient`, its `fn replay` and `fn resets_awaited_by`,
  and the policy functions `fn serving_pool`, `fn held_unspent` and `fn spent_resets` carry no
  visibility, so they are private to that module. Its `#[cfg(test)] pub(crate) mod test_support`
  builds clients for tests alone.
- Outside tests, the one crate-visible item in `src/capacity/client.rs` that builds a client is
  the constructor, `pub(crate) fn builtin_capacity_client_from_log`, which returns
  `Box<dyn CapacityClient>`. `TopologyRun::resumed` (`src/engine/topology/run.rs`) calls it.

What the census holds, in the tests of `src/capacity/client.rs`:

- `only_the_seam_reaches_capacity_from_the_engine` requires that in the production code of each
  file under `src/engine/` but the legacy writer's (`src/engine/coordinator.rs`,
  `src/engine/resume.rs`), with comments, literals and test-only items blanked, every
  word-bounded `capacity` that does not follow a `.` is followed by `::client`.
- `the_builtin_client_is_built_only_by_the_run` requires that, in the production code of the files
  under `src/`, the constructor's name occurs as a word once in `src/capacity/client.rs` and once
  in `src/engine/topology/run.rs`.

What neither holds:

- A crate-visible item added to `src/capacity/client.rs` itself that builds or exposes a second
  client: for example a `pub(crate) type` alias of `BuiltinClient`, or a `pub(crate)` function
  returning one. Code in that module may name its private items, the seam census accepts
  `capacity::client::<item>` whatever the item, and the constructor census counts only the word
  `builtin_capacity_client_from_log`.
- A name built by a macro: both censuses read the words a file writes (`word_occurrences` in
  `src/capacity/client.rs`), so a name a macro assembles from pieces is not seen.

## Why it is deferred

Round two's frontier pass graded the alias shape P2 (comment 7169622396). This file records it at
P3 under ruling r66, deferred to the change that adds a second `CapacityClient`, when the seam
first has something to protect.

## What the change that takes this up should do

Not proposed here; the guard names the change that takes it up.
