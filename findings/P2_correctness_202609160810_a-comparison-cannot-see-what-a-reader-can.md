---
id: PR283-COMPARISON-CANNOT-SEE-WHAT-A-READER-CAN
severity: P2
disposition: deferred
category: correctness
pr: 283
reviewed_sha: 3772b52bc2740ec67b7bb7e57c95c67e0313d22d
location: .github/scripts/test-pr-ready-audit.sh:2662, .github/scripts/test-pr-ready-audit.sh:2618, .github/scripts/test-pr-ready-audit.sh:3347 and .github/scripts/test-pr-ready-audit.sh:3037; its prose at .github/scripts/test-pr-ready-audit.sh:2743, :2860, :3941, :2697 and :3267
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
2. **Rebinding what no picture holds is not mutation.** The walk reaches an attribute's current object
   and pictures it. Where the binding sits in a dictionary, a list or an instance's own dictionary, that
   holder is pictured too, and rebinding moves it. But a function is walked and not pictured: a reader
   that **replaces** its `__annotations__` or its `__name__` leaves every picture as it was, so no
   picture moves, `moved` is empty, and restoration does not read the state again (`back`). A function
   is read only for its defaults, closure, dictionary and annotations, so a replaced `__name__` is not
   read at all. (An earlier revision said this of every rebinding; measured after round 20, a module
   name or an attribute rebound moves a picture and the state is read again -- item 4 of *Prose in
   the gate still broader than measured*, below.)
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
(proved in the pull request body), and line numbers in this file are at `03e77d4f` -- the same at
`3772b52b`, whose gate changes five lines of prose in place. Master's `1` is not a verdict about the
reader: master counts the parser's `json.loads(` spellings, and each reader adds one that a count
refuses. Each of the seven selectors and the control decodes with `object_pairs_hook=hook`, which is
not the spelling `object_pairs_hook=one_reading`, and fails the unhooked count --
`MUT-JSON-REPEATED-NAME-CHOSEN: got [1], want [0]`; each of the three correct readers adds a second
`object_pairs_hook=one_reading` and fails the hooked count -- `got [2], want [1]`. Re-executed on master
after round 20: exit `1`, and that one failing line, in each. (An earlier revision said every reader
added a second hooked spelling.)

| witness | where the marker or the wait lives | `aff2b024` | `eda3686e` | `03e77d4f` |
|---|---|:-:|:-:|:-:|
| signed zero | a list holding `0.0`, written to `-0.0`, read with `math.copysign` | 1 | **0** | **0** |
| equal distinct string | replaced with an equal string, tested with `is` | 1 | **0** | **0** |
| `UserList([0.0])` | the same as signed zero, through a `collections.UserList`, which holds its list in `.data` and is not a `list` subclass | 1 | **0** | **0** |
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

**What decides each of the seven witness rows is its first column: no picture moved, so restoration
returned `[]` without reading the state again.** The control is the contrast: two of its pictures
moved, restoration read the state again and named the holder's dictionary, and after it `mode="loose"`
returns -- the marker is back. (An earlier revision said this of every row, the control's included.)
An earlier revision of this file said the fingerprints compared equal in every case; for the first
four they do not. The fingerprint records identities, so it differs where an object was replaced — but
restoration never consults it; it decides only which states `every_state` asks the twice-named
document in. For the function-annotation witness an explicit `back()` **does** name the replacement:
the comparison can see it and is not asked. For the other six it names nothing either. After
restoration, in all seven, calling the reader with `mode="loose"` raises: the marker is not back.
Re-executed after round 20 with the probe's own functions at `3772b52b`: every cell as above.

## Prose in the gate still broader than measured, at `3772b52b`

The fix-check lens of the review of record, at `3772b52b`, held each sentence round 18 wrote against
what was measured, and found four that the gate still states more broadly than its witnesses -- three
P2 and one P3. **All four are about what the gate says, not what it does:** `3772b52b`'s gate differs
from `eda3686e`'s in comments and docstrings only (round 18's prover, re-run after round 20). Each was
re-executed after round 20 before it was written here, with the probe's own functions -- its
definitions at `3772b52b` loaded with a module and no drive, then called directly -- and each process
exited `0`. Lines are at `3772b52b`.

1. **P2 -- `:2743` (`reduction`) and `:2860` (`copied`).** *"ONE as the standard library's copy
   protocol reads it, or None where that would call a method of MODULE's (`foreign`)"*; *"or None where
   that protocol would call a method of MODULE's (`foreign`)"*.
   - **Measured:** `foreign` looks only for the methods it names (`PROTOCOL`: `__copy__`,
     `__reduce_ex__`, `__reduce__`, `__getstate__`, `__setstate__`, `__getnewargs_ex__`,
     `__getnewargs__`, `__new__`). For a value of a class MODULE wrote that overrides a method the
     protocol calls and `PROTOCOL` does not name, `foreign` is `False` and both hand back a value,
     having run MODULE's method. Measured for four: a `list` subclass's `__iter__` runs twice in
     `reduction` and once in `copied`, and a `dict` subclass's `items` the same; a `list` subclass's
     `append`, and a `dict` subclass's `__setitem__`, once in `copied`. One snapshot of a module holding
     all four ran them 12 times. The lens measured `__iter__`, with the same counts; the other three are
     added here.
   - **True:** *"or None where the protocol would call one of the methods `foreign` looks for -- and
     other methods of MODULE's the protocol calls still run: a `list` subclass's `__iter__` or `append`,
     a `dict` subclass's `items` or `__setitem__`."*
2. **P2 -- `:3941` (`fingerprint`).** *"Two states with the same fingerprint are the same state AS FAR
   AS STATE READS ONE"*.
   - **Measured:** `_marks = [0]`, a snapshot, `append(1)` then `pop()`, a second snapshot. The items
     are `[0]` in both; the size `state` records for `_marks` (`sized`) is **48, then 72**; the
     fingerprints are **equal**. A mark blanks the words `churn` measures -- here bytes 16 to 39: the
     list's length, its item pointer and its room -- whatever size the value reports, where `back`
     blanks them only while the size is unchanged: an explicit `back()` from the first state names
     **`_marks`**, and so does `restore` of it. Same as the lens.
   - **True:** *"Two states with the same fingerprint read the same objects, with pictures that agree
     but for the words `churn` measures -- which a mark leaves out whatever size the value reports, so
     two states that differ only in the room a container has set aside are one state here, though `back`
     tells them apart."*
3. **P2 -- `:2697` (`placed`); the same claim at `:3054`–`:3056` (`written`) and `:6254`–`:6256` (the
   `guarded` stand-in).** *"nothing written in C places a key without asking the key"* -- *"there is no
   implementation in C that places one without asking it"*, *"nothing written in C does otherwise"*.
   - **Measured:** a `dict` and a `set` of keys of a class MODULE wrote, whose `__hash__` and `__eq__`
     are counted, and raise while the container is written, each emptied with `clear` and filled with
     `update` from a snapshot of its own kind, **put every key back, the same objects, with no call of
     either** -- with distinct hashes, and with two keys hashing to `1`. What `written` itself runs does
     ask: the class's `__setitem__` for each pair, and a set's `update` from a list, each called
     `__hash__` once per key. Same as the lens, which measured distinct hashes; the colliding keys are
     added here.
   - **What it costs is a red, not a pass:** `placed` refuses such keys, `written` returns `False`, and
     `restore` names what it could not write back -- `_mapping`, after a key was added to it, came back
     **`['_mapping']`**, the key still there. A mapping C could have put back without asking a key is
     reported not back.
   - **True:** *"and writing, as `written` writes, cannot: the class's `__setitem__`, and a set's
     `update` from a list, ask each key for its hash. A `dict` or a `set` filled from another of its
     own kind would not -- measured, both place their keys by the hashes the other holds and ask no key
     anything -- but `written` does not write that way."*
4. **P3 -- `:3267` (`restore`); the same claim at `:1825`–`:1826` (the section header) and
   `:6410`–`:6411` (what this does not reach).** *"a binding rebound away from a value the walk
   pictured leaves every picture as it was"*; *"moves no picture"*.
   - **Measured,** each binding rebound once, then `restore`:

     | rebound | pictures moved | `back` asked | `restore` | binding put back |
     |---|---|---|---|---|
     | a module name | `<namespace>` | yes | `[]` | yes |
     | a `SimpleNamespace`'s attribute | the instance and its `__dict__` | yes | `[]` | yes |
     | module `__annotations__` | `<namespace>` | yes | `[]` | yes |
     | an attribute of an instance of a class MODULE wrote | the instance and its `__dict__` | yes | names its `__dict__` | yes |
     | a class attribute | none | no | `[]` | yes -- written back whole |
     | a function's `__annotations__` | none | no | `[]` | **no**; an explicit `back()` names it |
     | a function's `__name__` | none | no | `[]` | **no** |
     | a list slot, `0.0` to `-0.0` | none | no | `[]` | **no** |

     The first three are the lens's. The fourth is named although its binding is back: the instance's
     key-sharing dictionary comes back from `written` combined, reporting 168 bytes where it reported
     264 -- the cost round 16 disclosed for `s20_instance_counter`, not a new one.
   - **True:** *"a binding no picture holds -- a function's `__annotations__` or `__name__` -- rebound,
     or a binding rebound to an equal atom, leaves every picture as it was; one held in a dictionary or
     in an instance's own dictionary moves that holder's picture, and the state is read again."*

**None of the four was measured letting a change past the gate unseen, and none moves this file's
grade.** Two say less than is so: `restore` reads the state again after more rebindings than item 4's
sentence allows, and item 3's write-back could be made without asking a key -- where the gate declines
to make it, the value is named, not passed over. Two promise more than the gate keeps. Item 1's promise
is one the gate itself takes back at `:3103` -- reading still runs MODULE's code -- and this file's
fourth shape records. Item 2's fingerprint decides only which states `every_state` asks the
twice-named document in; the comparison restoration runs, `back`, tells the two rooms apart and names
the list. **Not measured:** whether any reader can make use of `every_state` taking two states that
differ only in room for one. None of the four turns a sentence cited under *Why this is P2* back into a
claim that every change is named.

**And three errors in this file, corrected in place** -- the lens's fifth item, P3, each re-executed
first: master's `1` was put down to a second hooked spelling for every reader (*Measured*), `UserList`
was called a list subclass (`issubclass(collections.UserList, list)` is `False`), and the mechanism
table's summary said every row moved no picture, which its own control contradicts. The failure
sequence's second item said of every rebinding what item 4 above measures of some, and is narrowed the
same way.

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

And the prose: wherever it opens them, the slice should make each sentence under *Prose in the gate
still broader than measured* say what that entry's narrower sentence says.

## Why this is P2 and not P1

**This migration's judgement, and the reasoning is here so it can be overruled.** Both review lenses
labelled individual instances P1 while reviewing the pull request; the review of record, at
`eda3686e` and at `3772b52b`, found no P1. As a standing finding about a shipped gate the bar is the
owner's ruling of 2026-09-11: *a P1 blocks a merge only if it can happen in normal use, or someone
without push access can trigger it.*

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
