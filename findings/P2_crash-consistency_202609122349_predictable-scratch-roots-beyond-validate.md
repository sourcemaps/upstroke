---
id: PREDICTABLE-SCRATCH-ROOTS-BEYOND-VALIDATE
severity: P2
disposition: deferred
category: crash-consistency
pr: 278
reviewed_sha: 71d44285071993439b87da02046b32b307c49ad0
location: src/runner/host/tests.rs
provenance: pre_existing
first_bad: undetermined
guard: whichever slice next opens shared test infrastructure; the counts below say which file buys the most per visit
---

## Failure sequence

`PR104-VALIDATE-SCRATCH-DIRECTORIES-PREDICTABLE-AND-UNRECLAIMED` was repaired in
`src/validate.rs` only. The shape it named is not confined to that file, and this row records how
much of it is left, measured rather than estimated, so the class is a number instead of an
impression.

Two patterns, each stated as the command that produced it, both run over `git ls-files src`
(`*.rs` only) with comment-only lines skipped.

**A. A temporary root nobody can be sure they own.** A `std::env::temp_dir()` path whose naming
statement — the source line plus continuation lines up to the terminating `;` — names
`std::process::id()` and does not name `ulid`.

| | `origin/master` at `71d44285` | after the `src/validate.rs` repair, at `fc4c140f` |
|---|---|---|
| A: pid-derived temp paths, no ULID | **66** sites in **30** files | **55** sites in **30** files |
| A2: of those, created as a directory within six lines — a scratch *root* | **57** sites in **25** files | **45** sites in **24** files |

**Positive control for the pattern**, because a count nobody has checked against a known-present
case is not evidence: run against `origin/master`, A reports **12** for `src/validate.rs`, which is
exactly the number `PR104` recorded there.

Run against the repaired tree, A still reports **1** for that file, and that one is a **false
positive worth stating**, because it is the clearest illustration of what these patterns can and
cannot see: it is the regression test's stand-in for another process's directory, and its tag
carries a fresh ULID assigned one statement earlier, which a line-and-continuation pattern cannot
read. The path is unique, and it is taken with an exclusive `create_dir` that refuses an occupied
name — the guarantee is the refusal, not the name.

A2 reports **0** for that file, and the agreement with the honest residue is luck rather than a
smarter pattern: the stand-in's acquisition is a call to `ForeignRoot::acquire`, so no `create_dir`
token falls inside A2's six-line window even though a directory is created there. A pattern that
reads call sites rather than tokens would count it again. So the residue is **45 roots across 24
files** by A2's count and by the honest one, for two different reasons.

A pid is not a unique key. It repeats across containers and after wraparound, which is why
`PR7-SCRATCH-FIXTURE-LEAK` records a Windows suite failing on a "fresh" fixture that was not fresh,
and why `PR245-WINGUEST-IMAGE-SHIPS-FIXTURE-RESIDUE` measured 25,187 `upstroke-*` roots already in
the frozen Windows image.

**B. A pre-clean whose result is thrown away.** A line matching
`let _ = <path>remove_dir_all(`.

| | `origin/master` at `71d44285` | after the repair |
|---|---|---|
| B: discarded `remove_dir_all` results | **142** sites in **25** files | **141** sites in **24** files |

`src/runner/host/tests.rs` alone holds **47** of them — a third of the tree's total, and the single
biggest concentration by a factor of two. It has **zero** pattern-A sites: it already names its
roots with a ULID, so what is left there is the discarded result rather than the predictability.
That split matters, because `PR64-CLEANUP-003-SCRATCH-PRECLEAN` is explicit that *both* halves are
required and that a repair keeping either one keeps the sequence.

Of the B sites, **28** are the pre-clean-then-create shape specifically — a discarded
`remove_dir_all(&x)` followed within four lines by `create_dir` or `create_dir_all` of the same
binding — and **24** of those 28 build the path from a tag and the pid alone. Those 24 are the
exact sequence `PR64` describes, unrepaired.

**What these patterns do not catch**, stated so the numbers are not read as a closed set: a
pre-clean more than four lines from its create, or one whose create names a different binding; a
helper that pre-cleans a path its caller created; `remove_dir`, `remove_file` or a `.ok()`-suffixed
discard rather than `let _ =`; and a predictable name built from something other than
`std::process::id()` — a thread name and a nanosecond clock, for instance, which
`src/agent/proc/tests.rs` uses in two places and which pattern A counts as pid-derived while
pattern A's ULID exclusion does not clear.

## What the change that takes this up should do

**Not one pull request.** A tree-wide sweep cannot merge under the standing delegation and should
not be attempted as one change: `src/workspace.rs` is `[[legacy]]` in `effects/allowlist.toml` with
`invariants_preserved[1]` requiring its behaviour untouched, and `src/effects/tests.rs` is a
CI-contract test — an instrument. Both hold sites in the counts above.

**And every one of these files is more expensive than it looks**, for a reason the `src/validate.rs`
repair found by walking into it: `effects/allowlist.toml` describes several of these test modules in
present tense, naming the scratch shape and counting the denied calls per method. The row for
`src/rundir/tests.rs` says its `scratch` "is `std::env::temp_dir()` joined with a tag and the
process id, and … opens with `let _ = fs::remove_dir_all(&dir)` on that predictable path", and lists
`fs::create_dir_all` 38, `fs::remove_dir_all` 3, `fs::create_dir` 1 among 87 sites. Repairing that
helper — three lines — falsifies both, so the repair reaches an effect allowlist and stops being a
delegated merge. Whoever takes a file up should budget for the governance edit beside the code one,
and should expect the same to be true of `src/workspace_manager/fixture.rs`, whose row describes its
fixture naming the same way.

The mechanism itself is settled and needs no design: `crate::rundir::scratch_tree::acquire` names a
root with a fresh ULID, takes it with one exclusive `create_dir` so an occupied name is refused
rather than adopted or deleted, and returns a guard that reclaims the tree on drop and on unwind.
`src/connect.rs`, `src/workspace_manager/tests.rs`, `src/rundir/tests.rs` and now `src/validate.rs`
already call it. The work is per-file conversion, not invention.

Suggested order, worst first by what each visit buys: `src/runner/host/tests.rs` (47 B),
`src/agent/proc/tests.rs` (20 B, 5 A), `src/runner/container/census/tests.rs` and
`src/runner/container/tests.rs` (10 B each), `src/review.rs` (6 A), `src/util.rs` (5 A, 7 B),
`src/rundir/tests.rs` (1 A, 1 B — the smallest diff in the tree and the one that proves the
allowlist cost).

**Severity.** `P2` is this row's own judgement, not a reviewer's word. The harm for an individual
site is already carried at `P1` by `PR64-CLEANUP-003-SCRATCH-PRECLEAN`, and the leak half by
`PR7-SCRATCH-FIXTURE-LEAK`; what this row adds is the extent, which is what a planner needs and
neither of those carries. No new sighting is recorded here — `PR245`'s Windows measurement is the
nearest thing to one, and it is that row's.

---

**2026-09-25: both P1s this row leans on are closed, and the extent is re-measured.** The pull
request that closed `PR64-CLEANUP-003-SCRATCH-PRECLEAN` and `PR7-SCRATCH-FIXTURE-LEAK` did the
per-file conversion this row asks for over the fixture **pre-cleans**, so the two sentences above
that read "already carried at `P1`" no longer have a row to point at; the class this row names is
what is left after them, and it is smaller.

Re-run at `b10c07ffe8822c7eaa5dc8b5fc8ff2fa7faaa874` with this row's own two patterns, reimplemented from the definitions above
(a `temp_dir()` naming statement — the line plus continuations to the terminating `;` — that names
`std::process::id()` and does not name `ulid`; and a line matching
`let _ = <path>remove_dir_all(`), over `git ls-files src` with comment-only lines skipped. The
"before" column is the same script at `d724fb16`, not the `71d44285` figures above, so the two
columns are comparable to each other and not to that table:

| | `d724fb16` | this head |
|---|---|---|
| A: pid-derived temp paths, no ULID | 57 sites in 31 files | **18** sites in **12** files |
| A2: of those, a scratch *root* | 46 sites in 24 files | **8** sites in **4** files |
| B: discarded `remove_dir_all` results | 146 sites in 24 files | **107** sites in **14** files |

**What B still counts, and why that is the honest residue rather than an oversight.**
`src/runner/host/tests.rs` holds 47 of the 107 and is unchanged: its `scratch` is already
ULID-named and pre-cleans nothing, so every one of its 47 is a **teardown** at the end of a test
body, not the pre-clean-before-ownership sequence `PR64` described. The suggested order above put
that file first on its B count; on the split this row itself draws — predictability versus a
discarded result — it buys the least, because what is left there leaks only on the failing runs and
cannot delete another holder's content. The pre-clean-then-create shape B's four-line refinement
counted is **0** at this head, against 29 at `d724fb16` by the same script.

The figures above are those of the tree `b10c07ff` carries — the commit that wrote this
paragraph — re-run there after a rebase rewrote the commit they were first stamped with. At
`6137824d4c6b2294bed74bec9759ecec0446fb7c`, this branch's last commit to touch `src/`, B is
**106** in 14 files, 47 of them still `src/runner/host/tests.rs`'s — the Windows lint repair
deleted one discarded teardown from `src/runner/container/tests.rs` — and A, A2 and the four-line
shape are unchanged.

**The allowlist cost this row predicted came due and was not paid.** "Repairing that helper — three
lines — falsifies both, so the repair reaches an effect allowlist and stops being a delegated
merge." It does, and the pull request left `effects/allowlist.toml` alone for exactly that reason;
the stale clauses are filed as `PR321-ALLOWLIST-CLAUSES-DESCRIBE-THE-OLD-SCRATCH-SHAPE`.

---

**2026-09-26: A2 is 0, and the two patterns missed two roots of the class they were written for.**
#322's review found one fixture root left in pattern A's own shape
(`engine/topology/recover/tests.rs`'s `upstroke-pr7e-<pid>-<tag>-<ordinal>`), and the search that
followed found the rest of A2 and two roots neither pattern can see. All were converted by the
pull request that closes `PR7-SCRATCH-FIXTURE-LEAK`. The patterns re-run from the definitions
above, over `git ls-tree -r <rev> src`, comment-only lines skipped; A2 here is "a
`create_dir`, `create_dir_all`, `create_public_dir` or `create_private_dir` on the statement's
line or the six after it", which counts 44 and 7 where the table above counts 46 and 8 at the same
two revisions:

| | `d724fb16` | `67d9bd41` | `d5d15c3f` |
|---|---|---|---|
| A: pid-derived temp paths, no ULID | 57 sites in 31 files | 18 in 12 | **8** in 6 |
| A2: of those, a directory created within six lines | 44 in 24 | 7 in 4 | **0** |
| B: discarded `remove_dir_all` results | 146 in 24 | 106 in 14 | **77** in 10 |

**The eight A sites left**, each read: two are witnesses planting the name an old helper computed
(`rundir/tests.rs`'s `the_root_the_old_helper_would_have_taken`, `recover/tests.rs`'s
`the_root_the_pid_keyed_helper_gave_a_first_fixture`); two are paths never created
(`util.rs`'s `upstroke-util-absent-<pid>` under a `should_panic`, `runner/host/naming.rs`'s
`never_created_directory`, which asserts its own absence); one is `validate.rs`'s stranger
stand-in, whose tag carries a fresh ULID -- the false positive this row already names; and three in
`agent/proc/tests.rs` (`upstroke-proc-detached-`, `upstroke-stopped-child-`,
`upstroke-proc-custom-cont-`) carry a nanosecond clock beside the pid, so a later process does not
compute them. Those three keep an adopting `create_dir_all` and a discarded teardown -- B's class,
not A's sequence.

**What neither pattern saw**, exactly as the paragraph above on what they do not catch predicts:

- `engine/topology/startup/tests.rs` named its root `upstroke-startup-census-<name>` -- no pid at
  all, one name for every process -- and opened with a discarded `rundir::remove_public_husk` of
  it. Pattern A needs `std::process::id()` and B needs `remove_dir_all`; this is `PR64`'s sequence
  with neither token.
- `engine/topology/scaffold.rs` gave every scaffold run `RunPaths::new(<repo>,
  "01SCAFFOLD00000000000000AA")`, whose private half is the default private root: one fixed
  directory, `~/.upstroke/runs/01SCAFFOLD00000000000000AA`, that every scaffold run in every
  process wrote into and nothing removed. Not a `temp_dir()` path, so outside both patterns.

Both were found by a full suite run into a fresh `TMPDIR` with every top-level entry it created
logged (inotify) and every file it wrote outside that `TMPDIR` listed by mtime, which sees what a
run actually creates rather than what a line looks like. What that cannot see is a path only a
`cfg(windows)` or failing branch creates; the static patterns above are the complement.

**B's 77** are 74 in test code and 3 in `src/workspace.rs`'s production region. None is a
pre-clean by either pre-clean-then-create scan -- a discarded `remove_dir_all` followed by a create
of the same binding within four lines, and any `remove_dir_all` followed within 25 lines by a create
of it or of a value derived from it -- which report **0** at this head against 28 and 30 at
`a3767bcc`. `src/runner/host/tests.rs` still holds 47 of them.
