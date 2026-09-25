---
id: R5-MAIN-01
severity: P3
disposition: deferred
category: correctness
pr: 318
reviewed_sha: 6c5e093f1fb033f97c6d7e80bd647a9f89131fb7
location: src/effects/tests.rs:969
provenance: introduced_by_feature
first_bad: 5f8a0b53afcd356d230703e09f1e2de3a23af840
guard: the next change to the production-fence rule in `src/effects/tests.rs` (`holds_in_the_production_build`, `atom_in_the_production_build`): tell a malformed cfg literal from a valid but unmodelled cfg atom before crediting an allowance; until then the compiler refuses every source that reaches this (E0539), so no source that passes the gates is excused through it
---

## Failure sequence

The production-fence rule in `src/effects/tests.rs` decides whether a file-level `deny` of a
governed lint is excused by an allowance that some CI production build applies. Given this source,
the rule credits the allowance and excuses the `deny`:

```rust
#![deny(clippy::disallowed_methods)]
#[cfg_attr(any(all(), target_os = b"linux"), allow(clippy::disallowed_methods))]
mod m {}
```

1. The byte-string value `b"linux"` is not a string rustc accepts as a cfg value.
   `literal_token_value` (`src/effects.rs:2355`) decodes it to nothing, so
   `atom_in_the_production_build` (`src/effects/tests.rs:983`, via `:992`) answers `None`, unknown.
2. `holds_in_the_production_build`'s `Any` arm (`:969`–`:979`) returns `Some(true)` as soon as a
   sibling is true, and `all()` is always true. The unknown part never gets weighed.
3. `allowance_applies_in_a_production_build` credits the allowance, and
   `denies_the_production_build_could_forbid` does not name the `deny`.
4. rustc refuses the predicate with E0539, so no production build applies that allowance, or builds
   the file at all.

The same happens when the invalid operand comes before `all()`, and for all three governed lints. A
direct or negated invalid literal earns no credit at this head. A valid but unmodelled cfg flag and
a malformed literal need different treatment here: a known-true disjunct can settle the first, but
it cannot make the second compile.

**Why P3.** `cargo clippy` refuses every such source, so this gives no mutation that passes the
gates, and it does not restore the round-four P1. The checker is wrong only about sources the
compiler already rejects.

## Evidence

The reviewer is MAIN, round five of #318, `gpt-6-astra` at `max`, verdict PASS with this one P3
deferred under the owner's round cap. The report is quoted in full in
https://github.com/sourcemaps/upstroke/pull/318#issuecomment-5826150011.

- Its 63-case control combines 7 malformed-literal forms (byte string, char, bad escape, non-ASCII
  hex, Unicode surrogate, Unicode out of range, leading Unicode underscore), 3 lints and 3 shapes.
  Rustc rejected all 63 with E0539. The 21 direct and 21 negated cases earned no credit. All 21
  under `any(all(), ..)` earned credit.
- Its guard-level witness asks `denies_the_production_build_could_forbid` about the source above
  and its two variants. The direct form is named for all three lints; the `any` forms are named for
  none. The witness fails `a literal rustc refuses excused the deny`.

Evidence on the build box: `~/orch-pr10/reviews/pr-318r5/main-evidence/10-undecodable-literals.*`
and `13-invalid-any-guard-witness.*`. The reviewer's draft of this file is in
`main-evidence/findings/`.

## Provenance, measured

The reviewer's draft said `pre_existing` with no first bad commit: relative to round five's batching
commit `6c5e093f`, it is older. Relative to #318's merge base `5b16f727`, the finding is
`introduced_by_feature`, as #318's ledger row says: the guard does not exist at `5b16f727`. The row
names `c2f29951` as first bad, but a measurement contradicts that.

**The measurement.** At each commit, a git-archive copy outside the repository got one appended test
in `src/effects/tests.rs`. The test asked the unchanged `denies_the_production_build_could_forbid`
about the three shapes, with a `cfg_attr(test, allow(..))` control that must be named. Results, for
all three lints:

| Commit | Test-only control | `any(all(), b"..")` and `any(b"..", all())` | Direct `b".."` |
|---|---|---|---|
| `89099a7c`, where the guard is introduced | not named | credited | credited |
| `5f8a0b53`, which reads `cfg_attr` predicates | named | credited | credited |
| `79ee0325`, `8f1c133a`, `42c9bc4f`, `8d0fa24d` | named | credited | credited |
| `c2f29951`, which reads cfg values as rustc does | named | credited | **not** credited |
| `6c5e093f`, the reviewed head | named | credited | not credited |

`c2f29951` narrowed the defect to the `any` case; it did not introduce it. The first commit at which
the guard evaluates a `cfg_attr` predicate at all is `5f8a0b53`. Its test-only control is named, and
it already credits the `any` case, so `first_bad` is `5f8a0b53`. At `89099a7c` the same source is
credited too, but so is `cfg_attr(test, ..)`: that commit reads no predicate, so it cannot be said
to misread this one. Between the two, only `src/engine/topology/recover/tests.rs` changes under
`src/`, through the merge of `master`. The probe and its receipts are in
`~/orch-pr10/findings-gate5-deferred-evidence/r5-main-01-provenance/`: `README.md`, `run.sh`,
`probe-test.rs`, `summary.txt`, one `probe-<sha8>.log` per commit, and `SHA256SUMS`. Each counted
run exited 0 and has a `Compiling upstroke v0.1.0 (<its copy>)` line. The first pass, made without
the control, is kept in `run1-no-control/`. Its results agree with the second pass, and the second
pass supersedes it.

**What the probe establishes.** It establishes the guard's verdict at eight commits, for three
lints, on exactly four sources: the test-only control, `target_os = b"linux"`, and the two `any`
orders around that one literal. Among #318's commits, these eight include every commit that
changed `src/effects.rs` or `src/effects/` from `89099a7c`, where the guard first exists, to the
reviewed head. The ones not probed change neither. The first commit before `89099a7c` that touched
the effects code, `96aa472f`, has no guard. So:

- the `any` shapes are credited at every commit from `5f8a0b53` on;
- the direct literal stops being credited at `c2f29951`;
- before `5f8a0b53`, the guard does not tell a test-only `cfg_attr` from a production one.

**What it does not establish.**

- **Other literal forms at earlier commits.** Before the reviewed head it probed only the byte
  string, never the other six malformed forms or the negated shapes. Their behaviour at earlier
  commits is inferred from the code, not measured.
- **The compiler's side.** It did not run the compiler. That rustc refuses these predicates with
  E0539 is the reviewer's result at the reviewed head; the compiler's answer does not depend on
  this repository's commit, but this probe did not re-measure it.
- **Whether `89099a7c` is the same defect.** The same source earns credit there too, through a
  guard that reads no predicate. Calling `5f8a0b53`, not `89099a7c`, the first bad commit is a
  judgement about what this finding describes, the mis-evaluation of a predicate, and not
  something the probe measured.

## What the change that takes this up should do

Reject a malformed literal before deciding the predicate's truth value. Keep the deliberately
conservative treatment of syntactically valid unknown cfg atoms, where an unmodelled flag may still
be settled by a known-true sibling. The regression is the reviewer's guard-level witness: the `any`
shapes must be named, as the direct and negated ones already are. The 63-case control is its
compiler-backed twin.
