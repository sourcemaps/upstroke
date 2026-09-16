---
id: PR283-COMPARISON-CANNOT-SEE-WHAT-A-READER-CAN
severity: P2
disposition: deferred
category: correctness
pr: 283
reviewed_sha: 8aa4a9cc1f2f65b145ac41f9ca95dca23d6800ea
location: .github/scripts/test-pr-ready-audit.sh:2701 and .github/scripts/test-pr-ready-audit.sh:3144
provenance: introduced_by_feature
first_bad:
guard: the slice that next opens `.github/scripts/test-pr-ready-audit.sh`'s state walk — `described`, `back` or `ATOMS`
---

## Failure sequence

The probe establishes that a repeat starts where its run started by **reading every value the
module's state reaches and comparing it with a picture taken when the run began**. A value it cannot
show is back is reported `unproven`.

**Two things a reader can observe are not differences to that comparison, so a value that changed is
reported as restored** — not `unproven`, but positively certified:

1. **Equality is not identity.** Atoms are compared as `(type, value)`. `(float, 0.0)` equals
   `(float, -0.0)`; two equal-but-distinct strings compare equal while `is` tells them apart.
2. **Rebinding is not mutation.** The walk reaches an attribute's current object and pictures it.
   A reader that **replaces** the binding leaves the pictured object untouched, so `moved` is empty
   and restoration skips verification entirely.

A first-call selector whose marker is carried that way therefore survives the repeat: the repeat
never re-drives the genuine first call, the unhooked decode is never exercised, and a document
naming `findings` twice returns `{"verdict": "PASS", "findings": []}` with the complete gate at
exit `0`.

**A third shape is the same defect in the read rather than the comparison:** reading a value can run
the module's own code. Two instances were repaired in round 17 (a list subclass's `__len__`, a
dictionary key's `__hash__`); one was measured and left, below.

## Measured

All executed against the complete gate, `master aff2b024` / head, by the two independent review
lenses and by round 17.

| witness | where the marker lives | exits |
|---|---|---|
| signed zero | a list holding `0.0`, written to `-0.0` | `1 / 0` |
| equal distinct string | replaced with an equal string, tested by `is` | `1 / 0` |
| `UserList([0.0])` | same, through a list subclass | `1 / 0` |
| `__annotations__` replaced | `{"return": 0}` → `{"return": 1}` | `1 / 0` |
| `__name__` replaced | `"_tally"` → `"used"` | `1 / 0` |

In each case `restore_report` was `[]` and fingerprints compared equal. **For the annotation
witness an explicit `back()` call DOES detect the difference — restoration never asks it, because
`moved` is empty.** That is the sharpest statement of the defect: the comparison can see it and is
not consulted.

**Reported and not repaired, same mechanism, measured at two heads:** `u05_getattr_under_lock`, a
holder that fills itself under a lock the module holds. The walk asks every value it reaches whether
it carries code (`elsewhere` ends with `getattr(value, "__code__", None)`), and **asking runs a
Python `__getattr__`**. Complete gate: **no end at 900 s**, at `571fbaf2` and at this head. The
rule does not enumerate `elsewhere`, and the bound does not fire because the block lands in
`attempt`'s snapshot, inside `recorded`.

## What the change that takes this up should do

**Decide what the comparison is for, rather than adding a third spelling of equality.** Five rounds
of this file's history say an enumeration loses: rounds 12–16 each closed every named instance and
each was followed by a new one, and the escapes grew more exotic rather than fewer — a counter in a
return annotation holding a `threading.Semaphore`, list room observable through `__sizeof__()`,
signed zero.

Three directions, none free:

- **Compare identity where identity is observable.** Record `id()` alongside `(type, value)` for
  values a reader could test with `is`. Sound for the two witnesses above; it will red a correct
  reader that legitimately rebinds an equal value, and that cost must be measured before it ships.
- **Verify the binding, not only the bound object.** `back()` already detects the annotation
  witness; restoration does not call it because `moved` is empty. Making verification unconditional
  closes the rebinding half without touching equality — and costs a full re-read on every restore.
- **State the bound instead of closing it.** Say in the gate and here that the probe compares
  observable value and not identity or binding, that reading state runs some module-declared code,
  and that a reader constructed against these facts is not detected. **This is what the gate's own
  comments already do for its other limits**, and it is the option this finding exists to make
  available rather than to pre-empt.

**Whichever is chosen, the residual must be named where a reader of the gate will see it.** An
unnamed gap in a check that reports `unproven` for everything else reads as a guarantee.

## Why this is P2 and not P1

**This migration's judgement, and the reasoning is here so it can be overruled.** Both review lenses
labelled the individual instances P1 while reviewing the pull request. As a standing finding about a
shipped gate the bar is the owner's ruling of 2026-09-11: *a P1 blocks a merge only if it can happen
in normal use, or someone without push access can trigger it.*

Exercising any witness here requires **adding a reader to `scripts/pr-review-parse.py` in the
repository**, which requires push access. It changes nothing about how the gate judges any pull
request written without one. **It weakens an argument about how much of the gate a reviewer must
read** — which is the same reasoning `SHAPE-GATE-MISSES-ARITHMETIC-SUBSTITUTION` records for its
own scanner, and that row is P1 because of **its claim of enforcement**. This gate states its bound;
that is the difference, and if the bound is ever dropped from the comments this row should be raised.
