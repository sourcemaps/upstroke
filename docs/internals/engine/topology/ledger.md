# `src/engine/topology/ledger.rs`

Extended notes for [`src/engine/topology/ledger.rs`](../../../../src/engine/topology/ledger.rs).
[Source on GitHub](https://github.com/sourcemaps/upstroke/blob/master/src/engine/topology/ledger.rs).
The relative link works in a checkout or on GitHub; the GitHub link also works from the published site.

The code is the authority for what it does. The explanatory prose is preserved below.
Each backticked part of a section heading is an exact source excerpt. Search for the final
excerpt within the preceding item when a heading names both an item and a line inside it.

## Module

`decisions.resource_accounting`, transcribed: the twenty-eight rows with their enforcement
domains, the five outcome equations row by row from each row's `at_run_end`, an observation of
every row from a fold plus a physical inventory plus the process-local rows, and a check that
reads each requirement against a before/after pair — and, for the rows whose
`resource_accounting` cell names several facts (R14, R21, R27), one requirement per named fact
(`Expectation::parts`), each consumed by the verdict. ST-09 for the durable rows; the process-local
rows (R3, R4, R13, R17, R22, R28) are observed here as the loop reports them and certified at G6.

Nothing here measures the disk: [`PhysicalInventory`] is filled by the test that owns the
fixture (`recover/tests.rs`, `ledger_inventory`), and this module only says what the numbers
must be. That keeps the equations one transcription with one reader.

**A late instance** (A-12: the PR11 decision appendix's §5.2, under O4(a), adopted on 2026-10-11 as
an agent decision under the owner's delegation; the finding `FUC-D5-ACCOUNT`). Terminal
finalization's last step sweeps every earlier incarnation's instance before it removes the
execution root (the follow-up C record, §5.4). An earlier incarnation's Git writer that is still
running can act after that sweep has scanned, and the instance it creates is a late instance: it
belongs to its slot's row (R9, R10 or R24), classed `ResumablyOpen` — never used, and reclaimed
with force by the run's next terminal finalization or resume, whose final sweep finds it — and the
objects its `HEAD` or index references belong to it while it exists. The ledger admits one only as
an entry the final sweep's own discovery takes for an instance, a directory with a valid instance
name and never a name alone (A12-R3), and only on two pieces of evidence: an at-sweep inventory,
taken, that does not list it, and its incarnation attributed to this run. Every other case reads as
a disagreement, failing closed. The late facts arrive in [`LateInventory`], filled by the test that
owns the fixture as the physical inventory is (`ledger/tests.rs` for A-12's own evidence); the
census over them is this module's.

## `pub enum Row {`

R1 through R28, in the packet's order. [`Row::resource`] is the row's name and [`Row::domain`]
its enforcement domain; both are transcribed, and the domain test holds the counts to the
packet's twelve, twelve, one and three.

## `pub enum Class {`

The packet's five classes plus `Void`: Halted's "released (void)" rows hold whatever the fold
holds and nothing reads them, so the class is named rather than folded into `Released`.

## `pub enum Fact {`

What an observation found: a count held or zero, an artifact present or absent, a ledger
balanced or not.

`Late { crossed, attributed }` is a late part's fact: how many of the row's instances crossed the
final sweep — no intent names them, and an at-sweep inventory was taken that does not list them —
and how many of those the census attributes to an earlier incarnation of this run (the appendix's
`late_task_instances`, `late_staging_instances` and `late_snapshot_instances`). It counts as
`crossed`.

## `pub enum Requirement {`

What an equation asks of a before/after pair. `Zero` and `Absent` read the after fact alone;
`Present` requires something after; `Retained` requires the after fact to equal the before one,
which is what "resumably_open" and "persistent_output" mean for a row whose count the fixture
chose; `Balanced` the process-local ledgers; `Monotone` the consumed counters (never reused,
so never smaller); `Any` the void rows.

A-12 adds three. `Attributed`, a late part's: admitted as zero, or as the count the census
attributes — its two counts equal — so a late part with an instance not attributed to an earlier
incarnation is its disagreement. `AbsentUnlessLate`, R18's in an ended outcome: absent, or present
only when the three late parts' attributed counts sum above zero. `MonotoneUnlessLate`, R27's
`unreachable_objects` in an ended outcome: after ≥ before − `late_referenced_objects`, in place of
plain `Monotone`. The last two read facts beyond their own pair, so `admits_beside` takes the after
ledger's [`LateTotals`], and `admits` reads every requirement as if nothing were late.

## `pub struct LateTotals {`

The after ledger's late facts that `AbsentUnlessLate` and `MonotoneUnlessLate` read: the
attributed late instances over R9, R10 and R24, and R27's `late_referenced_objects`.

## `pub struct Expectation {`

One row of an outcome's equation: the row's class, its requirement, one requirement per named fact
(`parts`, each with its own class), and `kept`, the class the row is accounted in instead of
`class` when a late instance keeps or recreates the resource (A-4). Only R18 in an ended outcome
has one.

## `pub fn class_beside(&self, after: Fact, late: LateTotals) -> Class {`

The class a row is accounted in for its after fact: `kept` when that fact is `Present` beside at
least one attributed late instance of the after ledger, `class` otherwise. It reads the facts
`AbsentUnlessLate` admits a present root on, so the root `check` admits beside a late instance is
classed `ResumablyOpen`, and an absent root, or a present one beside no late instance, `Pruned`.
`check` and `record` report a row's class through it. Until #337's first review (A12-R1) the class
was the equation's alone, and the export classed the root late instances keep `Pruned`.

## `pub struct ProcessLocal {`

R3, R4, R13, R17, R22 and R28 as the process reports them: the invocation ledger balanced, the
entitlements held, the run lock held, a surviving cleanup hold observed.

## `pub struct PhysicalInventory {`

The disk, Git and container facts the external rows read: slots by namespace (intents and
directories as one set), refs and pins under the run's namespace, the integration ref, the
execution root, the run directory's files, the private records, the two lock files, container
intents, the credential-volume check, R21's outputs one by one (the report, the normalized plan,
the question, answer and `.partial` files, the marker, the lock files, the owner and commit
records, the private artifacts), and R27's objects — the ones the last pre-finalization
observation saw referenced, the whole store as it listed it, and what `fsck` reports
unreachable, so that nothing present before the run end is gone after it, an
already-unreachable object included.

## `pub struct InstanceSeen {`

One entry under a slot namespace, as the observer found it: its row, its name, whether it is a
directory, and the objects its `HEAD` or index references. `directory` is read without following a
link (`symlink_metadata`), as the final sweep's discovery reads an entry (`namespace_instances`,
`src/workspace_manager.rs`): a regular file, a link, an entry of any other kind and one whose
metadata cannot be read are not directories. It is a raw fact like the others, and the census, not
the observer, decides which entry is an instance. Until #337's second review (A12-R3) the census
read the name alone, so a regular file at a valid instance path, with no intent or registration,
counted as a late instance that the next finalization leaves in place.

## `pub struct LateInventory {`

What A-12 adds to the physical inventory, as raw facts: every entry under the three slot
namespaces, of whatever kind, with whether it is a directory and the objects its `HEAD` or index
references (`InstanceSeen`), the intent files the inventory found, the at-sweep inventory's
listing when one was taken, the intent records of every inventory the ledger took of the run, and
the before-inventory's unreachable objects.

**Why a struct of its own.** [`PhysicalInventory`] keeps its fields because the frozen
`ledger_inventory` (`recover/tests.rs`) builds it with an exhaustive struct literal: a field added
there would change that frozen observation, which the appendix reserves to a decision (O6). So the
late half travels beside it, and [`observe`] reads an empty one.

**The at-sweep inventory** (APX-A12LATE) is the listing the test's hook observer takes at the
`Before` phase of the final sweep's first removal, through the manager's effect-hook seam: every
funnel calls `EffectHooks::phase` before and after its site. The sweep scans before it removes
anything, so an instance that listing does not list was published after the sweep's scan.

## `pub const LATE_ROWS: [(Row, &str); 3] = [`

The three rows that hold slot instances and their namespaces' directory names; `LATE_PART` and
`LATE_OBJECTS` name the parts A-12 adds.

## `struct Census {`

The census's answer for one inventory: per row, the instances that crossed the final sweep and
those attributed; the objects an attributed late instance references again; and whether an
at-sweep inventory was taken. `slots` is a row's main fact, the namespace's intents and worktrees
less the crossed ones; an inventory that counts fewer than crossed reads `Unbalanced`, failing
closed.

## `fn earlier_incarnations(`

An earlier incarnation (APX-A12ATTR) is one whose id `run_started` or a `run_resumed` records, or
one an intent of this run records in any inventory the ledger took of the run: `IntentRecord`'s
`incarnation`, read where its `run_id` is the run's. An incarnation can start a Git writer and never
reach `run_resumed`: recovery recreates open generations at step (g) before it records its
incarnation. But every add writes its intent, synced, before it starts its Git child, so such an
incarnation left an intent naming it, which the before-inventory of its successor's command reads
before that successor's reclaim removes it. That is why the ledger takes its before-inventory at
the start of every command, a command that fails included, and why attribution reads the union of
the run's inventories. No other durable record serves without a change: the creation records carry
only the creating incarnation. The answer is the set of those incarnations' instance tags.

## `fn take_census(events: &[TopologyEvent], late: &LateInventory) -> Census {`

The late predicate: an entry under a slot namespace that is an instance, that no intent names, and
that the at-sweep inventory, taken, does not list, has crossed the final sweep; it is attributed
when its tag is an earlier incarnation's of this run. An instance is what the final sweep's
discovery (`namespace_instances`) takes for one, and so what the run's next finalization reclaims:
a directory, read without following a link, whose name is a valid slot instance's
(`SlotInstance::from_entry`, and `Slot::validate` read through `Slot::id`). Without an at-sweep
inventory — the sweep removed nothing, or none was taken — no instance is late.

Everything else is residue, never late, and stays in its row's main fact, whose `Absent` makes it a
disagreement; a root that residue alone keeps stands beside no late instance, so R18 disagrees too.
That covers an entry that is no instance — a regular file, a link, an entry of any other kind, a
directory whose name is no valid instance's, and an entry whose metadata could not be read — none
of which the discovery's walk of the namespaces takes for an instance (it passes over the first
four and refuses the sweep on the last), so that with no registration naming it no finalization
reclaims it (A12-R3); the census certifies none of them either way. It also covers an earlier
incarnation's intentless instance the at-sweep inventory lists and finalization left, residue the
sweep could discover; one published between the sweep's scan and the observation is listed and
reads the same way. `late_referenced_objects` counts the before-inventory's unreachable objects
that an attributed late instance's `HEAD` or index references.

## `pub struct Ledger {`

`at_sweep_inventory` says whether the command the ledger observes took an at-sweep inventory; the
export prints it.

## `pub fn observe(`

One observation per row, with the evidence a disagreement quotes. R1 is the fold's own
pipeline count; R14 is one fact per consumed counter — the merge sequence, task keys, display
ids, generations, attempts, lineage indexes, repair budget units, verification defers (the
`merge_verification_unavailable` events whose outcome is `Deferred`, counted from the durable
prefix the observer is handed — each is one unit consumed; a queue entry's `defers` is the same
number while its candidate is queued and leaves with the entry at `task_merged`, so summing the
queue, as the part did until PR10's round 4, gave a consumed unit back at publication and read a
successful lifecycle as non-monotone (the round-4 contract lens, F1;
`consumed_verification_defers_survive_publication`); a task's `defers` is its worker backoff, and
until round 2 the part summed that instead) and override slots — each held monotone; R21 one fact per persistent output and R27 one per object
accounting (`Expectation::parts`); R20 is balanced when no effect site names a volume and the
recorded volume map is unchanged (operator-owned by classification).

It is [`observe_late`] with an empty [`LateInventory`]: no at-sweep inventory, so no instance is
late, every late part is zero, and every row reads as it did before A-12.

## `pub fn observe_late(`

The census first, then every row. R9, R10 and R24's main facts count every other intent and
worktree, as before; each row gains the part `late_instances`, and R27 gains the part
`late_referenced_objects`. R18's evidence names the late instances beside the root.

## `fn parts_of(`

A row's own parts, each classed as its row is. Only the late parts carry another class.

## `pub fn equation(outcome: Outcome) -> Vec<Expectation> {`

The outcome equations. Complete: everything released, consumed or pruned. Parked and
BudgetExceeded: the queue, the leases, the candidates refs and the questions retained
(resumably open), the pins and worktrees pruned, the root pruned. Halted: the void rows `Any`,
the candidates refs retained (forensic). NoRunFinished: every row retained, the execution root
present — "a command ended by the append-error protocol leaves exactly this shape".

In Complete, Parked, Halted and BudgetExceeded a late instance is the exception (A-6): R9, R10 and
R24's main facts stay `Absent` and their late parts are admitted, classed `ResumablyOpen`; R18 is
absent, classed `Pruned`, or present only beside a late instance, which keeps or recreates it, and
then classed `ResumablyOpen` (A-4); R27's unreachable objects may fall by what a late instance
references again. NoRunFinished is unchanged.

## `pub fn equation(outcome: Outcome) -> Vec<Expectation>` › `let late = |row| Expectation {`

R9, R10 and R24 in an ended outcome: the main fact `Absent`, and `late_instances`, classed
`ResumablyOpen`, `Attributed`.

## `pub fn equation(outcome: Outcome) -> Vec<Expectation>` › `let root = Expectation {`

R18 in an ended outcome: `Pruned`, `AbsentUnlessLate`, and kept `ResumablyOpen`. A-4 amends its
`at_run_end` for Complete and Halted to "pruned unless a late instance keeps or recreates it (then
resumably_open)", and for Parked and BudgetExceeded to "pruned unless a later resume recreates it,
or a late instance keeps or recreates it (then resumably_open)"; a later resume's recreation comes
after the run end this ledger observes, so at the run end the four read alike. It is one fact, not
a part: the appendix's equation states R18 "`Absent`, or `Present` only when the three late parts
sum above zero". NoRunFinished's R18 stays `ResumablyOpen`, `Present`.

## `pub fn equation(outcome: Outcome) -> Vec<Expectation>` › `let consumed = Expectation {`

The rows whose `resource_accounting` cell names several facts carry
one requirement per named fact, and the verdict consumes each.

## `pub fn equation(outcome: Outcome) -> Vec<Expectation>` › `let outputs = Expectation {`

The report is derived at every run end and regenerated on resume, so
it is present at every outcome but the mid-run one; the marker is
removed after `run_started`; the lock files persist; answer, question
and `.partial` files are never pruned; the private artifacts persist.

## `pub fn equation(outcome: Outcome) -> Vec<Expectation>` › `let released = Expectation {`

Released to Git: nothing a pruned ref, pin or worktree referenced is
gone, nothing the store held before the run end is gone, and what
fsck reports unreachable can only grow.

## `pub fn equation(outcome: Outcome) -> Vec<Expectation>` › `let mut released_parts = parts_of(`

In an ended outcome the unreachable count may fall by `late_referenced_objects` (A-5: an object a
late instance's `HEAD` or index references again is accounted by that instance's row while it
exists, and returns to R27 when the instance is reclaimed), which is a part of its own, classed
`ResumablyOpen`.

## `pub fn check(before: &Ledger, after: &Ledger, outcome: Outcome) -> Vec<Disagreement> {`

Every expectation against the pair; a missing observation is a disagreement too. The requirements
that read other facts read the after ledger's [`LateTotals`], and a part's disagreement carries the
part's class. A row's disagreement carries the class `class_beside` gives its after fact, which for
a disagreeing R18 is `Pruned`; a missing observation keeps the equation's `class`.

## `pub fn record(before: &Ledger, after: &Ledger, outcome: Outcome) -> LedgerRecord {`

The ledger as a document, one row per expectation with both facts and the evidence, rendered
as a Markdown table by [`LedgerRecord::render`] for the record. Each row carries the class it is
accounted in (`class_beside`): the root a late instance keeps is exported `ResumablyOpen`, a
pruned one `Pruned`. Each part line carries its class, the late parts among them, per row and
outcome, and the table closes with whether an at-sweep inventory was taken (A-12's export).
