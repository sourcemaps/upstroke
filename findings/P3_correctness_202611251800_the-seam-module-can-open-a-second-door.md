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
    -> that client answers capacity questions the client the run installed never sees

What the compiler holds:

- `src/capacity.rs` declares the module `pub(crate) mod client;`, so it is crate-private.
- In `src/capacity/client.rs`, `struct BuiltinClient`, its `fn replay` and `fn resets_awaited_by`,
  and the policy functions `fn serving_pool`, `fn held_unspent` and `fn spent_resets` carry no
  visibility, so they are private to that module. Its `#[cfg(test)] pub(crate) mod test_support`
  builds clients for tests alone.
- Outside tests, the one crate-visible item in `src/capacity/client.rs` that builds a client is
  the constructor, `pub(crate) fn builtin_capacity_client_from_log`, which returns
  `Box<dyn CapacityClient>`. `TopologyRun::resumed` (`src/engine/topology/run.rs`) calls it.

What the census holds, in the tests of `src/capacity/client.rs`. Both rules read the production
files, every `.rs` file under `src/` but the whole-file test modules, and in each its production
code: its source with comments, literals and test-only items blanked.

- Rule one, `only_the_seam_reaches_capacity_from_the_engine`, reads the files whose path below the
  repository root has `src` and `engine` as its first two components, except the legacy writer's
  two (`src/engine/coordinator.rs`, `src/engine/resume.rs`). It requires every word-bounded
  `capacity` in their code to be followed, after optional whitespace, by `::` and then, after
  optional whitespace, by a word-bounded `client`. It reads nothing before the word.
- Rule two, `the_builtin_client_is_built_only_by_the_run`, requires the constructor's name to occur
  as a word once in `src/capacity/client.rs`, once in `src/engine/topology/run.rs`, and in no other
  file.

What neither holds:

- A crate-visible item added to `src/capacity/client.rs` itself that builds or exposes a second
  client: for example a `pub(crate)` function returning one,
  `pub(crate) fn second_client() -> Box<dyn CapacityClient> { Box::new(BuiltinClient::default()) }`.
  Code in that module may name its private items, rule one accepts `capacity::client::<item>`
  whatever the item, and rule two counts only the word `builtin_capacity_client_from_log`.
- A name built by a macro: both rules read the words a file writes (`word_occurrences` in
  `src/capacity/client.rs`), so a name a macro assembles from pieces is not seen.
- One naming called twice. Rule two counts words, not calls. `src/engine/topology/run.rs` can name
  the constructor once, binding it to a local
  (`let build = crate::capacity::client::builtin_capacity_client_from_log;`), and call the local
  twice: the run builds two clients, and the counts are the ones rule two accepts.
  `the_constructor_census_sees_a_second_client` plants this and asserts that rule two accepts it.
- A re-export outside the engine. Rule one reads no file outside `src/engine/` and resolves no
  path or name. A file outside the engine can declare a module alias that re-exports the capacity
  module (`pub(crate) use crate::capacity as cap;` in `src/util.rs`), and an engine file then
  reaches the module around the seam (`crate::util::cap::exhausted_pools_of`) without writing
  `capacity`. `the_capacity_census_sees_a_bypass_and_ignores_prose` plants this and asserts that
  rule one accepts it.
- The exempt files' schema-4 dispatch. The legacy writer's exception is by file, so rule one reads
  nothing in `src/engine/coordinator.rs` or `src/engine/resume.rs`, the schema-4 dispatch at the
  head of each included: `if opts.writes_topology() {` in `run_harness`
  (`src/engine/coordinator.rs:60`) and `if let Some(root) = super::topology::entry::route_resume(opts)? {`
  in `resume_harness` (`src/engine/resume.rs:50`). A schema-4 run or resume enters through one of
  them, and each file already imports the module (`use crate::capacity;`), so code added inside
  either branch can reach it around the seam with no rule refusing it.

## Why it is deferred

Round two's frontier pass graded this door P2 (comment 7169622396). This file records it at P3
under ruling r66, deferred to the change that adds a second `CapacityClient`, when the seam first
has something to protect.

Round three's two passes graded one naming called twice P2 each, both witnessed. The frontier pass
graded the module alias P3, witnessed, and the regression pass counted it inside its P2. Ruling r69
records both here as limits of the census, beside the exempt files' schema-4 dispatch, and files no
new P2: a census of words cannot count calls or resolve aliases, and ruling r66 dropped the parser
that tried. Until that ruling this file's example of a second door was a `pub(crate) type` alias of
`BuiltinClient`, which round three's regression pass noted; it is a `pub(crate)` function now.

## What the change that takes this up should do

Not proposed here; the guard names the change that takes it up.
