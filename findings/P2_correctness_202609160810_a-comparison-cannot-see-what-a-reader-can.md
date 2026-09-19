---
id: PR283-COMPARISON-CANNOT-SEE-WHAT-A-READER-CAN
severity: P2
disposition: deferred
category: correctness
pr: 283
reviewed_sha: eda3686e3fd642da3df8418d8937920ad346cb0b
location: .github/scripts/test-pr-ready-audit.sh:2662, .github/scripts/test-pr-ready-audit.sh:2618, .github/scripts/test-pr-ready-audit.sh:3347 and .github/scripts/test-pr-ready-audit.sh:3037
provenance: introduced_by_feature
first_bad:
guard: the slice that next opens `.github/scripts/test-pr-ready-audit.sh`'s state walk — `ref`, `kept`, `restore`, `contents` or the bound in `perform`
---

## Failure sequence

The probe establishes that a repeat starts where its run started by **reading every value the
module's state reaches and comparing it with a picture taken when the run began**. A value whose
picture moved and that cannot be put back is reported `unproven`.

**Three things a reader can observe are not differences to that reading, so a value that changed is
passed over** — not reported `unproven`: the repeat runs from a state that is not the one it repeats,
and nothing says so.

1. **Equality is not identity.** Atoms are compared as `(type, value)` (`ref`). `(float, 0.0)` equals
   `(float, -0.0)`; two equal-but-distinct strings compare equal while `is` tells them apart.
2. **Rebinding is not mutation.** The walk reaches an attribute's current object and pictures it. A
   reader that **replaces** the binding leaves every pictured object untouched, so no picture moves,
   `moved` is empty, and restoration does not read the state again (`back`). A function is read only
   for its defaults, closure, dictionary and annotations, so a replaced `__name__` is not read at all.
3. **What a module-level `__dunder__` name holds is not read.** `kept` leaves out of the walk every
   `__dunder__` binding in the module's namespace that holds no function of the module's: the names
   the interpreter keeps there, and `__annotations__`, which the module's own code writes.

A first-call selector whose marker is carried any of these ways therefore survives the repeat: the
repeat never re-drives the genuine first call, the unhooked decode is never exercised, and a document
naming `findings` twice returns `{"verdict": "PASS", "findings": []}` with the complete gate at exit
`0`.

**A fourth shape is the same defect in the read rather than the comparison: reading a value can run the
module's own code, and where that code waits, nothing ends it.** Measured in two places. An
`OrderedDict`'s own `items`, which `contents` reads it with, finds each key by its hash: every key's
`__hash__` runs, and its `__eq__` where two hashes collide (a plain `dict`'s runs neither); so do its
copy and the copy protocol's reading of its iterator. And `elsewhere` asks every value for `__code__`,
which runs a Python `__getattr__`. The bound reaches only the snapshot `perform` takes: a repeat's own
snapshot and restorations run inside `recorded`, where `expired` never raises, and the sweep's walk
(`held`) runs with no timer armed. A **correct** reader whose key or holder waits on a lock the module
holds gets no report, and the gate does not end.

## Measured

Every exit below was executed in round 18 against the complete gate: the reader appended to the
parser in a `git archive` of the named commit, the whole gate run from the tree root under a
900-second bound and a 6 GiB cap. `03e77d4f` changes `eda3686e`'s gate in comments and docstrings only
(proved in the pull request body), and line numbers in this file are at `03e77d4f`. Master's `1` is not
a verdict about the reader: appending any reader adds a second literal hooked spelling, which master's
count refuses.

| witness | where the marker or the wait lives | `aff2b024` | `eda3686e` | `03e77d4f` |
|---|---|:-:|:-:|:-:|
| signed zero | a list holding `0.0`, written to `-0.0`, read with `math.copysign` | 1 | **0** | **0** |
| equal distinct string | replaced with an equal string, tested with `is` | 1 | **0** | **0** |
| `UserList([0.0])` | the same as signed zero, through a list subclass | 1 | **0** | **0** |
| function `__annotations__` replaced | `{"return": 0}` → `{"return": 1}` | 1 | **0** | **0** |
| function `__name__` replaced | `"_tally"` → `"used"` | 1 | **0** | **0** |
| module `__annotations__` → `collections.UserString` | `__annotations__ = {"reader_state": UserString("")}`, `.data` grown | 1 | **0** | **0** |
| module `__annotations__` → UTF-8 incremental decoder | its state set to `(b"\xc2", 0)` on the first call | 1 | **0** | **0** |
| control: that `UserString` under an ordinary name | `_reader_annotations = {"reader_state": ...}` | 1 | 1 | 1 |
| `OrderedDict` whose keys' `__eq__` waits on a held lock — **correct** reader | two keys hashing to `1` | 1 | **no end** | **no end** |
| `OrderedDict` whose key's `__hash__` waits on a held lock — **correct** reader | one key; no collision needed | 1 | **no end** | **no end** |
| `u05_getattr_under_lock` — **correct** reader | a holder whose `__getattr__` takes the lock | 1 | **no end** | **no end** |

At both heads every `0` is the gate's parser line reading `decoded=yes unrefusing=- unproven=-
skipped=-`, and the control reads `unrefusing=extra_review_reader
unproven=_reader_annotations[reader_state].__dict__`. Each `no end` wrote no report; its probe, dumped
at the bound, was blocked in `attempt → perform → replay → recorded → state`, in `contents` at the key's
`__eq__` or `__hash__`, or in `elsewhere` at the holder's `__getattr__`. The probe run directly on a
module with no `main`, holding the same `__getattr__` holder, blocks instead in `sweep → held →
elsewhere`, where no timer is armed.

Called directly, each of the seven selectors returns `{"verdict": "PASS", "findings": []}` on its first
call with `mode="loose"`, and each of the three correct readers raises on the duplicate and returns both
single-name readings.

**The mechanism, re-measured with the probe's own functions** — its definitions loaded with the module
and no drive, then `state()`, the reader's first call, `restore`, a fresh `state()` and an explicit
`back()`:

| witness | pictures that moved | `restore` | explicit `back()` | fingerprints, before and after |
|---|---|---|---|---|
| signed zero | none | `[]` | `[]` | unequal |
| equal distinct string | none | `[]` | `[]` | unequal |
| `UserList([0.0])` | none | `[]` | `[]` | unequal |
| function `__annotations__` | none | `[]` | **`_tally.__annotations__`** | unequal |
| function `__name__` | none | `[]` | `[]` | equal |
| module `__annotations__`, both holders | none | `[]` | `[]` | equal |
| control, ordinary name | two paths | names `_reader_annotations[reader_state].__dict__` | names it | equal |

**What decides every row is its first column: no picture moved, so restoration returned `[]` without
reading the state again.** An earlier revision of this file said the fingerprints compared equal in
every case; for the first four they do not. The fingerprint records identities, so it differs where an
object was replaced — but restoration never consults it; it decides only which states `every_state`
asks the twice-named document in. For the function-annotation witness an explicit `back()` **does**
name the replacement: the comparison can see it and is not asked. For the other six it names nothing
either. After restoration, in all seven, calling the reader with `mode="loose"` raises: the marker
is not back.

## What the change that takes this up should do

**Decide what the comparison is for, rather than adding a third spelling of equality.** Rounds 12–17
each closed every instance named and were each followed by a stranger one — a counter in a return
annotation holding a `threading.Semaphore`, list room observable through `__sizeof__()`, signed zero,
then module `__annotations__` and an `OrderedDict` whose keys wait on a lock.

Three directions, none free:

- **Compare identity where identity is observable.** Record `id()` alongside `(type, value)` for
  values a reader could test with `is`. It will red a correct reader that legitimately rebinds an
  equal value, and that cost must be measured before it ships.
- **Verify the binding, not only the bound object.** Making `back()` unconditional costs a full re-read
  on every restore — and, measured, it names only the function-annotation witness of the seven.
- **State the bound instead of closing it.** Round 18 took this one: the gate now says what its
  reading cannot see and where its bound does not reach, at the lines cited below.

The read-side residue needs its own decision: a reading of an `OrderedDict`'s order that asks no key —
none of its `items`, `__iter__`, copy or iterator reduction does — or naming such a mapping unproven
unread; and a bound that reaches the snapshot and restorations `attempt` makes and the sweep's walk,
which `recorded` and the sweep leave unbounded today.

## Why this is P2 and not P1

**This migration's judgement, and the reasoning is here so it can be overruled.** Both review lenses
labelled individual instances P1 while reviewing the pull request; the review of record at
`eda3686e` found no P1. As a standing finding about a shipped gate the bar is the owner's ruling of
2026-09-11: *a P1 blocks a merge only if it can happen in normal use, or someone without push access
can trigger it.*

Exercising any witness here requires **adding a reader to `scripts/pr-review-parse.py` in the
repository**, which requires push access. It changes nothing about how the gate judges any pull
request written without one. **It weakens an argument about how much of the gate a reviewer must
read** — the same reasoning `SHAPE-GATE-MISSES-ARITHMETIC-SUBSTITUTION` records for its own scanner,
and that row is P1 because of **its claim of enforcement**.

At `eda3686e` this gate still made that claim: its docstrings said a value that is not back is named
rather than passed over, and that reading the state asks no key anything — so by this paragraph's own
reasoning the row was a P1. **At `03e77d4f` the gate states its bound instead**, where a reader of it
will see it:

- `.github/scripts/test-pr-ready-audit.sh:1824` — what the reading cannot see is passed over, not named;
- `:2613` — `kept`: what a module-level `__dunder__` name holds, `__annotations__` included, is not
  walked, pictured or compared;
- `:2658` — `ref`: an atom is not held by its identity;
- `:3028` — `contents`: an `OrderedDict` asks its keys for their hash, and for `__eq__` on a collision;
- `:3103` — `state`: reading still runs the module's code, and the bound reaches one snapshot;
- `:3265` — `restore`: where no picture moved the state is not read again, which is not the same as
  the state being back;
- `:3369` — `perform`: its snapshot is the only reading the bound reaches, and a reading that waits
  anywhere else never returns;
- `:6404` and `:6417` — the section on what this does not reach: the comparison's bound, and where
  the gate does not end, each with its witnesses.

**If those sentences are ever dropped, or turned back into a claim that every change is named, this
row should be raised.**
