---
id: PR335-ABSENT-WITNESS-CANDIDATE-EXCEEDS-PATH-LIMIT
severity: P2
disposition: deferred
category: portability
pr: 335
reviewed_sha: e62c8d8fcf538c08346d3a687b27ab08d13e23a0
location: src/runner/host/naming.rs:408
provenance: pre_existing
first_bad: 7a95c6516f0fa1767b73722ed5c42be1209eb5ec
guard: this change, branch fix-P2/portability_the-absent-directory-witness-candidate-exceeds-the-path-limit, which files this finding and repairs it in the same pull request
---

## Failure sequence

`a_directory_that_is_merely_absent_is_walked_past_to_a_later_match` (`src/runner/host/naming.rs:408`) puts a
directory it never creates ahead of the test binary's own directory on `PATH`, and requires the search to walk past
the first and find the binary in the second. Its never-created directory is `never_created_directory("walked-past")`
(`:291`), a name under the temporary directory.

1. `TMPDIR` is an absolute 4,027-byte path.
2. The never-created directory is the temporary directory plus 51 bytes and the process id's digits, 4,084
   bytes here, and `symlink_metadata` answers `NotFound`, so the fixture's own absence check passes.
3. The search asks that directory's candidate, the directory plus `/` and the test binary's 25-byte file
   name: the temporary directory plus 77 bytes and the process id's digits, 4,110 bytes here. Linux refuses a
   path of 4,096 bytes or more with `ENAMETOOLONG` (os error 36) before any lookup, however absent its parent is.
4. `is_program` maps only `NotFound` to absence (`:76`), so `resolve_program` rightly stops at that candidate with
   `UpstrokeError::Filesystem` (`:220`): the policy `an_undetermined_candidate_stops_the_search_before_a_later_match`
   (`:372`) pins, and it stays as it is.
5. The witness's `.expect("a not-found miss continues the search to the installed program")` panics (`:420`): it
   fails on unchanged source, though the program it must find is where it always was.

**Executed at `5355b829`**, the base of this change and master when it started, holding #335's merge (`naming.rs` blob
`bfdfe43b` there, as at `e62c8d8f`), built under this change's own iso and run with exactly `TMPDIR`,
`CARGO_TARGET_DIR` and `PATH` set:

| construction | `TMPDIR` bytes | process id digits | this witness | log sha256 |
|---|---|---|---|---|
| L3 | 3,975 | 6 | ok | 627f115cbf65cb69… |
| L4 | 4,000 | 6 | ok | 64d9482d8e412e43… |
| R0 | 4,008 | 6 | ok | 5a0ca44bc5befa4f… |
| W0 | 4,027 | 6 | FAILED | 6194419cc40984f8… |

The 4,027-byte run's panic names a 4,110-byte candidate and `code: 36, kind: InvalidFilename, message:
"File name too long"`. With a 6-digit process id the threshold is a temporary directory of 4,096 − 77 −
6 = 4,013 bytes; the three controls sit below it and pass.

**Where it was found.** At `e62c8d8f`, #335's round-3 head: the 4,027-byte constructions of #335's round-3
repair worker, and the round-3 regular and platform lenses' 4,027-byte cases; under the lenses' 4,000- and
4,008-byte constructions it passed, and the stop witness passed at 4,027. #335 left `naming.rs` byte-identical, so
the defect is not that change's, and #335's triage assigned it to a separate change whose scope permits this witness:
this one.

**First bad: `7a95c651`, by Git.** `git log -S` names it as the commit that introduced this witness,
`never_created_directory` and `this_test_binary`, and its parent has none of them. The test module is byte-identical
from it to `5355b829`; the production region differs only by `#![deny(` becoming `#![forbid(` (a lint level, no
behaviour); and `KeyCase::same_key`, which the search uses to find `PATH` in the composed environment, is unchanged.
Derived from history, not executed at that commit.

**Severity: P2, by analogy with `P335-R2-PL-1`**, the same failure class in the PATH-entry fixture of
`src/runner/host/tests.rs`, which #335's round-2 platform lens rated P2 portability: a witness on a required leg goes
red on unchanged source under a long temporary directory, and its red reads exactly like a regression of the policy
it witnesses.

## What the change that takes this up should do

Make this witness's verdict independent of the temporary directory's length while keeping its meaning, that an
absent directory is walked past to a later match; keep production program resolution byte-identical and the stop
witness's meaning, that an undetermined candidate stops the search. Construct the absent candidate so that no
ambient path's length can decide it, for example within the path budget its later match already needs, as
`P335-R2-PL-1`'s repair did for the fixture; state why the construction is deterministic; and sweep `naming.rs`'s
witnesses for the same class. Skipping the case, swallowing an error and weakening an assertion are not repairs, and
no remote SMB server may be depended on.
