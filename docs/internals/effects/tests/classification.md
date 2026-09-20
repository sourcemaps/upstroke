# `src/effects/tests/classification.rs`

Extended notes for [`src/effects/tests/classification.rs`](../../../../src/effects/tests/classification.rs).

The code is the authority for what it does. These notes started as the module's source prose.
Each code fragment in a heading is an exact source substring. When a heading names an enclosing
item before `›`, find that item first, then the following fragment within it.

## Module

`(3) Wrapper classification`: the five checks that hold
`effects/wrappers.toml` against the tree it classifies.

The domain derivation, both directions of the effectful/denied
correspondence, the crate paths the denials are built from, the funnel rows,
and the `libc::` sweep. All five *read* -- `effects/wrappers.toml`,
`clippy.toml`, and the modules those two name -- and none of them writes
anything or starts a process.

Everything they read with stays where it was. The schemas and readers
(`ModuleClassification`, `wrappers`, `denylist`, `scanned_sources`,
`repo_root`) are `super`'s, and the production scanners
(`externally_reachable_fns`, `production_code`,
`blank_comments_and_strings`) are `crate::effects`'. This file consumes
them; it re-derives none of them.

**No name here is a test name.** The five `#[test]` wrappers stay in `super`
under the harness names the contract and CI know, and the five functions
below are deliberately named otherwise -- so `--list` over the test binary is
unchanged and nothing nests under `effects::tests::classification`.

### Why the bodies sit inside a `cfg(test)` module

A file reached by a plain `mod` declaration is inside every whole-tree
census's domain. That is the constraint `policy.rs` records, and the one
that kept the effectful build helpers out of it. The inline module closes it
here for both of the repository's source cutters at once:
[`crate::effects::production_region`] truncates at the first `#[cfg(test)]`
and [`crate::effects::production_code`] excises the item that attribute
attaches to, so the four bodies are outside both regions and this file reads
as the test logic it is.

It does so **without moving the whole-file module census**.
`census_domain::declared_whole_file_test_modules` derives a skip only from a
**terminated** declaration -- `mod name;` -- and an inline module with a
body opens a scope the scan reads declarations *inside* rather than naming a
file of its own. So
`the_whole_file_test_modules_are_resolved_from_the_declarations_not_the_file_names`
still resolves `cfg::WHOLE_FILE_TEST_MODULES` and no pinned test is renamed.
Measured, not argued, and re-measured when W1 grew the set: declared the
other way, this file joins that test's named set as a seventh
`["agent/proc/test_support/readiness.rs", "effects/tests/classification.rs",
"engine/topology/scaffold.rs", "events/log/premove.rs",
"rundir/scratch_tree.rs", "runner/container/fake.rs",
"workspace_manager/fixture.rs"]` against its expected six, and the
comparison below it resolves one module more than
`WHOLE_FILE_TEST_MODULES` lists.

That terminated form is deliberately not spelled out here, for the reason
`policy.rs` gives: one written inside a comment is the exact shape that once
derived a phantom skip and removed a real file from every census below it,
and the blanking that now defeats it is not a reason to write another.

The neighbour that makes the shape legible is
`src/runner/container/census/tests.rs`, whose bare `this_file_is_test_only`
marker module closes the *region* half only -- `production_code` excises the
marker and then scans that file in full -- and what keeps it out of the
whole-tree censuses is a real declaration one level up. This file can have
neither, so it wraps the bodies rather than marking above them.

The `#![deny]` below deliberately stays **above** the cut. Blanking takes
the prose, so that attribute is all three whole-tree walks' per-file "this
region is empty" control has left to count here -- and a region that
collapses to nothing is exactly what that control exists to catch.

The three effect denials are **restored** rather than inherited. `super`
allows them because it drives a compiler over fixtures it creates; nothing
in this file does, so the allowance has no business reaching it. Measured
rather than believed: one probe -- a `println!`, a `std::fs::write` and a
`std::process::Command` -- is refused three times here and emits no
`disallowed_*` at all from the identical lines in `tests.rs`, so the `deny`
is load-bearing and not a restatement of an ambient rule. That is also what
keeps this module out of `effects/allowlist.toml`: an allowance is what that
file records, and this module takes none.

## `pub(super) mod checks` › `pub(in crate::effects::tests) fn reachable_fns_are_classified() {`

Every externally reachable `fn` of a legacy or shared module is classified.

The domain is **derived from the modules**, not listed: a `pub fn` added to
one of them fails this test until somebody decides what it is. That is the
only half of `mechanism` (3) a test can hold — the classification itself is
a review — and it is the half that omission attacks.

## `pub(in crate::effects::tests) fn reachable_fns_are_classified() {` › `let classified: Vec<&str> = module`

A row may carry its receiver (`Workspace::branch_exists`) so the
denied path can name it; the domain is over bare fn names.

## `pub(super) mod checks` › `fn classification_disagreement(`

One module's record against one source text: the names the domain derives that
no class holds, the names a class holds that the domain does not derive, and
how many were derived. Pure over the text, so that the census above and the
regression test below judge a mutated source exactly as they judge the file.

## `pub(super) mod checks` › `fn unpinned_shared_names(module: &ModuleClassification, source: &str) -> Vec<String> {`

**A name is not a callable.** The record classifies by bare name -- the
comparison above strips every row to its last path component, and
`externally_reachable_fns` returns a set -- so two callables that share a name
are one row, whatever each of them does, and a denial in `clippy.toml` names
one path. The review of `409a6138` (`PR309-INLINE-WRAPPER-NAME-COLLISION`) used
it: a `pub(crate) fn run` calling `std::fs::write`, in an attribute-free inline
module of `src/engine/coordinator.rs`, referenced from a production body under
`engine::topology`, passed clippy and the whole effects suite, because `run`
was already an `effectful` row -- the entry point -- denied as
`upstroke::engine::coordinator::run`, which is not the path of the second one.
Renamed to a name nothing else bore, the same function failed the census as
unclassified. The exemption was the collision.

What the census promises is that a callable **the source writes as
`fn name`** cannot be added to a classified module without the record
changing. A new name changes it. A second bearer of an old name did not, and
now does: for every derived name,
`effects::reachable_fn_multiplicity` counts the `fn` declarations in the
file's production code that bear it, and every count above one must be pinned
in the module's `shared` table, exactly. The pins are not a list of
suspicions: 69 names in 22 modules are shared today, all of them ordinary --
`fmt` and `drop` for several types, `#[cfg(unix)]` and `#[cfg(windows)]`
twins, a trait's declaration and its impls -- and the table says how many
callables each row was written for. A pin that outlives its callables is
refused too, so a count cannot be left high for the next arrival.

**Chosen over carrying full identity through the record**, which is the other
way out the review named. Identity would mean deriving an owner for every
function -- its `impl`, its trait, its inline module -- and rewriting some 760
rows and their denial check to carry it: a rebuild of shared enforcement
machinery whose blast radius is every classified module, to buy one thing the
count does not. What the count does not see is a swap -- one bearer removed
and another added under the same name in the same change -- and a swap is an
edit to a callable the record already answers for, which is reviewed like any
other change to a classified body; no census reads bodies.

**What neither reading sees is a callable the text does not spell, and that is
executed, not hypothetical.** The second review of #309 put a macro in
`src/engine/coordinator.rs` that emits `pub(super) fn $name(..)` calling
`std::fs::write`, invoked with a fresh name: the text holds `fn $name`, so the
function is no classification obligation and no multiplicity increment, it
compiles under the coordinator's existing allow, and a topology caller reached
it with clippy at exit 0 and 185 tests passing; with the name written
literally the census fails. An aliased `include!` does the same from a file no
scan reads. This census does not close that and does not claim to:
`PR7-WRAPPERS-EMPTY-DOMAIN` is restored, deferred, with the project owner, and
`PR7-CLASSIFICATION-DOMAIN-READS-FNS-ONLY` records the `const`/`static` form.

## `pub(super) mod checks` › `fn names_in_two_classes(module: &ModuleClassification) -> Vec<String> {`

The bare names a record holds more than once, across its four classes. The
census refuses any; it is a value so that the regression test below can show
that a row for a second path is one.

## `pub(super) mod checks` › `fn bearers_of_one_path(owners: &[String]) -> bool {`

Whether every bearer of a name is reached by one path: all declared at the
same place (`#[cfg(unix)]` and `#[cfg(not(unix))]` twins are -- two items of
one name in one module cannot otherwise coexist), or all the declaration and
impls of one trait under one enclosing path, which a call reaches through the
trait and a denial names through the trait. Owners come from
`effects::reachable_fn_owners`.

## `pub(super) mod checks` › `fn effectful_names_shared_across_paths(`

**An effectful name is shared only by bearers of one path.** Round 1 offered
the pin as the way to admit a second bearer of a classified name, and said
that raising it asserted "every effectful one is denied by its own path" -- a
review duty. The second review of #309
(`PR309-SHARED-EFFECTFUL-PIN-CANNOT-RECORD-ITS-DENIAL`) showed the duty cannot
be discharged: a second `run_with`, in an inline module of the coordinator,
writing a file, is refused unpinned; with `shared = { run_with = 2 }` added,
clippy exits 0 and 185 tests pass, the second path undenied; and the honest
record cannot be written, because a denial for the second path is a denial no
row classifies, and a row for it strips to `run_with`, a name in two classes.
So the pin is refused where no honest record exists: for every `effectful`
row borne more than once, the bearers must be one path. The three the tree
shares today are: `rundir::hold_cleanup_lease_for_child` is a `cfg` pair, and
`GitView::materialize` and `GitView::discard` are a trait's declaration and
its impl. `effect_free`, `funnel` and `effectful_unnameable` names may still be
shared across paths, because one row is an honest record for them: none of
them is denied by path.

## `pub(super) mod checks` › `fn denials_no_row_classifies(record: &Wrappers, denied: &ClippyToml) -> Vec<String> {`

The `upstroke::` denials that no `effectful` row accounts for, as a value:
the reverse half of `effectful_wrappers_are_denied`, which asserts it empty,
and the first of the two record controls below.

## `pub(super) mod checks` › `pub(in crate::effects::tests) fn the_shared_effectful_pin_witness_and_its_controls() {`

The second review's witness and its three controls, against the real
coordinator, the real record and the real denylist, in memory. Unpinned, the
second `run_with` is refused by the count. Pinned -- the review's one-line
record edit -- the count is satisfied, which is the witness, and the one-path
rule refuses it, which is the repair. Then the two record controls: with a
denial for the second path added, that denial is one no row classifies; with
a row for it added too, the denial is accounted for and `run_with` is a name
in two classes. Last, the legitimate shapes: every module of the real record
passes the one-path rule, more than two effectful names are in fact shared,
and `bearers_of_one_path` answers five owner lists as stated.

## `pub(super) mod checks` › `pub(in crate::effects::tests) fn shared_names_are_pinned() {`

`unpinned_shared_names` over every recorded module, with a floor of forty
pinned names so that a multiplicity reading that finds nothing cannot pass.

## `pub(super) mod checks` › `pub(in crate::effects::tests) fn the_collision_witness_and_its_renamed_control() {`

The review's witness and its control, held against the real
`src/engine/coordinator.rs` and its real record, in memory. The head is clean
and `run` is an `effectful` name borne once. With the witness appended -- the
review's module, verbatim -- the name census finds nothing, which is the
defect, and the multiplicity census refuses exactly one thing: two callables
bear `run`. With the same function renamed `rf_unique_effect`, the name census
finds exactly that name unclassified and the multiplicity census is silent. So
either spelling of the function is refused, each by the check that owns it.

## `pub(super) mod checks` › `pub(in crate::effects::tests) fn effectful_wrappers_are_denied() {`

"effectful wrappers are added to the disallowed list themselves".

## `pub(in crate::effects::tests) fn effectful_wrappers_are_denied() {` › `let path = format!("{}::{name}", module.crate_path);`

`Type::method` is recorded as written, so an inherent method keeps
its receiver in the path clippy has to resolve.

## `pub(in crate::effects::tests) fn effectful_wrappers_are_denied() {` › `let classified: BTreeSet<String> = record`

The other direction: every crate-internal denial is a row somebody
classified. A `upstroke::…` entry nobody classified is a denial with no
review behind it.

## `pub(super) mod checks` › `pub(in crate::effects::tests) fn crate_paths_name_the_modules() {`

`crate_path = ""` is the record's escape hatch: `effectful_wrappers_are_denied`
refuses an effectful row it cannot path, so an empty path pushes every
effectful body of that module into `effectful_unnameable`, the class the
denial check skips. #306 found three private `mod`s of `engine` using it on
the claim that a private module has no clippy path -- and a `pub(super)` fn
there is visible to every module under `engine::topology`, and clippy
resolves the path: a reference to `crate::engine::attempt::run_attempt` from
`engine::topology::integrate` was undenied, then refused once the path was
listed. So the hatch is the binary crate root's alone, and every other path
is derived from the file it classifies, so a denial built from it resolves
to the module it names rather than to nothing.

## `pub(super) mod checks` › `pub(in crate::effects::tests) fn funnel_rows_name_a_site() {`

A row classified `funnel` really does name a site.

Read from [`crate::effects::production_code`] since #306, the region the
classifier's domain reads. `Supervisor::begin` and `finish`, the Terminate
site's funnel rows in `src/agent/proc.rs`, are declared below that file's
first `#[cfg(test)]` (line 958, inside `windows_job`), where the truncating
region could not see them: the #306 regression review measured a funnel row
for `begin` failing with `declares no such fn` under the old reader. A funnel
row and the domain it belongs to are now read from one region.

## `pub(in crate::effects::tests) fn funnel_rows_name_a_site()` › `let path = format!("{}::{name}", module.crate_path);`

A funnel is not a wrapper: it must not also be denied.

## `pub(super) mod checks` › `pub(in crate::effects::tests) fn libc_items_are_classified_and_denied() {`

Every `libc::` item the tree names is classified effect or not-an-effect, and
every one classified an effect is denied.

`claim_scope` makes exhaustiveness "the disallowed list is complete for the
**primitives the crate uses**", so the list is derived from the tree rather
than transcribed from the sentence's `fork/kill/setpgid/setsid/flock/fcntl/
exec*` — which is six names out of the twenty-four this crate actually calls.

## `pub(in crate::effects::tests) fn libc_items_are_classified_and_denied() {` › `let effects: BTreeSet<&str> = record.libc.effect.iter().map(String::as_str).collect();`

The other direction, or a reclassification would be free: moving an item
from `effect` to `not_an_effect` would leave its denial in place with
nothing behind it, and the first assertion could not tell.
