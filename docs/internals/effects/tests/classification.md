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

## `pub(super) mod checks` › `fn bearers_of_one_path(borne: usize, owners: &[Vec<OwnerScope>]) -> bool {`

Whether where the bearers of a name are declared shows them to be one path. It
is a whitelist of two shapes, and everything else is refused. The places read
have to be as many as the callables `reachable_fn_multiplicity` counted: an
empty list is a reading that went quiet and not a name with no second path,
and one place for two callables would otherwise be, trivially, one scope.

First, every bearer has to be reached through inline `mod`s alone: each scope
above the one it is written in must have been read as `mod name`. That refuses
anything inside a block, a function body, or the braces, parentheses or
brackets of a macro, whose contents can end up anywhere, without having to
recognise which of those it is. Then:

1. **One scope.** Every bearer is written directly in the same braces -- the
   same opening byte, or all of them at the file's top level -- and those
   braces were read as a module, a trait or an impl. Two items of one name in
   one scope can only be `cfg` twins, and twins in one `impl` block share its
   receiver whatever it is called, so this shape needs no name at all.
2. **One trait, declared beside its impls.** Every bearer is directly inside a
   `trait Name` declaration or an `impl Name for ..`, all of them written in
   one and the same enclosing scope, under one `Name`, and at least one of
   them the declaration. A call reaches each through the trait and a denial
   names it through the trait. The declaration is required because it is what
   makes `Name` mean something here without resolving it: an item declared in
   a scope cannot share its name with an import in that scope, so beside
   `trait Name` the word `Name` is that trait. That holds where the declaration
   is compiled, which a reading that does not evaluate `cfg` takes on trust.

**Same is a position, never a spelling.** The first form of this function took
the owners as label strings and accepted them when they were equal; the review
of `993f080d` made two different receivers spell alike twice over (the notes
of `effects::reachable_fn_owners` have both). So a false refusal is accepted
as the price: two `#[cfg]`-exclusive `impl A` blocks each holding a twin are
one path and are refused, and the author writes the `cfg` on the functions
inside one block instead. What this is not is callable identity. It judges
the declarations the source writes, and it is the caller that decides which
names are judged at all.

## `pub(super) mod checks` › `fn effectful_names_shared_across_paths(`

**An effectful name is shared in two shapes only.** Round 1 offered the pin as
the way to admit a second bearer of a classified name, and said that raising
it asserted "every effectful one is denied by its own path" -- a review duty.
The second review of #309
(`PR309-SHARED-EFFECTFUL-PIN-CANNOT-RECORD-ITS-DENIAL`) showed the duty cannot
be discharged: a second `run_with`, in an inline module of the coordinator,
writing a file, is refused unpinned; with `shared = { run_with = 2 }` added,
clippy exits 0 and 185 tests pass, the second path undenied; and the honest
record cannot be written, because a denial for the second path is a denial no
row classifies, and a row for it strips to `run_with`, a name in two classes.
So the pin is refused where no honest record exists: for every `effectful`
row borne more than once, `bearers_of_one_path` has to hold of where its
bearers are declared. The count comes from `reachable_fn_multiplicity` and the
places from `reachable_fn_owners`, and the rule is handed both, so an owner
reading that goes quiet is a complaint and not a pass. The
three the tree shares today are: `rundir::hold_cleanup_lease_for_child` is a
`cfg` pair at its file's top level, and `GitView::materialize` and
`GitView::discard` are a trait's declaration and its impl, written beside each
other at theirs. `effect_free`, `funnel` and `effectful_unnameable` names may
still be shared across paths, because one row is an honest record for them:
none of them is denied by path.

**Refusing every shared effectful pin was measured and is not available.**
Those three rows are the cost: two of them are a trait method, whose
declaration and impl cannot bear different names, and all three are in files
this pull request does not touch.

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
in two classes. Last, the tree itself: every module of the real record passes
the rule, and more than two effectful names are in fact shared, so it judged
something.

The three texts it edits are read with their line endings normalised to LF.
The anchors it inserts after end in a newline, and a Windows checkout holds
CRLF: the first push of this test failed on the Windows guest at the first
anchor, `left: 0, right: 1`, and nowhere else, which is the platform the
hosted matrix only links for.

## `pub(super) mod checks` › `fn one_path(source: &str, name: &str) -> bool {`

The reading and the rule together over one source text, for the shape tables
below. It asserts first that the name is borne more than once, so a fixture
the reading finds nothing in fails as a broken fixture and not as a refusal.

## `pub(super) mod checks` › `fn headers(chain: &[OwnerScope]) -> Vec<OwnerHeader> {`

What a chain's headers were read as, without where they open.

## `pub(super) mod checks` › `pub(in crate::effects::tests) fn the_spelling_defeats_and_their_controls() {`

The third review's two defeats and the control of each, against the real
`src/engine/resume.rs` and its real record, in memory; the shapes are the
reviews' own text. For all four the record is the one the reviews wrote -- the
name pinned at two, the first receiver's method an `effectful` row, both set
on the parsed record so that no anchor in its text has to hold -- and it
satisfies the name census and the count, which is what made it a witness.
Then two things are asserted of each, and the pair is the point. **What the
headers spell is the same for both bearers**: `Unread` and `Unread` under the
const-generic braces, `Unread`, inherent impl under the two `const _` blocks,
and the same again for each control, since an inherent impl no longer carries
a name to differ by. **And the pin is refused all four times**, because the
two bearers open at different bytes. Before the repair the defeats passed and
the controls failed, which is the verdict following the spelling.

The rest holds each decision of `bearers_of_one_path` from both sides. Seven
legitimate sharings pass -- twins at the top level, in an inline module and in
one `impl`; a trait with generics, supertraits and a `where` clause beside two
impls; an `unsafe` trait behind a `pub(in ..)`; a trait and its impl inside
one inline module; a trait beside its impl for `[u8; 4]`. Fifteen shapes are
refused, each chosen so that one decision alone refuses it: two impls (position, not header); two blocks of one
spelling (the same, and the false refusal the rule accepts); one impl inside a
block (the way down is modules alone); twins directly inside braces whose
header is unread (the scope has to be placed); an impl written away from its
trait's declaration (one enclosing scope); impls with no declaration beside
them; a declaration beside another trait's impl; a trait named by a path; a
free function and a method; a trait's method and an inherent one; a bearer in
a macro's body; twins in a macro invocation's braces, and in its parentheses;
a bearer in one's square brackets beside a free function; a bearer nested in a
method of the impl. An empty owner list is refused, and so is a list shorter
or longer than the count it is judged against.

## `pub(super) mod checks` › `pub(in crate::effects::tests) fn the_owner_reading_places_each_header_or_leaves_it_unread() {`

`effects::reachable_fn_owners` on nineteen one-bearer sources, by the headers
it reads: nested inline modules behind three visibilities, inherent impls with
and without an `->` in their generics, a trait's impl under two attributes, an
`unsafe` and a `where` clause, a trait's impl for `[u8; 4]` (the `;` is inside
brackets and ends nothing), a trait with a supertrait written against its name
and an `unsafe` one with generics -- and the ones it must leave `Unread`: a
trait named by a path, an `impl const` whose `for` is not its second word, a
const argument's braces, a `const _` block, a function whose parameter is
`impl Sized` (the reading at `993f080d` answered `impl Sized)` for it,
measured), a macro's body, the parentheses and the square brackets of a macro
invocation, an `extern` block. The file's top level is the empty chain, and a
closer with no opener of its own -- not Rust, but a reading that lost its
place would look like it -- closes nothing: the function after a stray `)` and
`]` inside `mod a` is still in `mod a`.

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
