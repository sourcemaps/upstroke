# `src/effects/tests/ci_model.rs`

Extended notes for [`src/effects/tests/ci_model.rs`](../../../../src/effects/tests/ci_model.rs).

The code is the authority for what it does. These notes started as the module's source prose.
Each code fragment in a heading is an exact source substring. When a heading names an enclosing
item before `›`, find that item first, then the following fragment within it.

## Module

What CI runs, on which runners, and with which shells — as constants.

**One authority, deliberately.** The workflow oracle in `super::workflow`
checks that `.github/workflows/ci.yml` matches these; the cfg census in
`super::cfg` decides which of [`CI_TARGETS`]'s runners compiles a predicate.
A runner list written down twice is a list where one copy gets updated, and
the copy that does not is the one that quietly stops measuring — with
nothing failing to say so. Both readers take the same table.

The three effect denials are **restored** here rather than inherited.
`super`'s module-level allowance exists because that file drives
`clippy-driver` over fixtures it has to create; this module names constants
and performs no effect at all, so the allowance has no business reaching it.

## `pub(super) const CI_WORKFLOW: &str = ".github/workflows/ci.yml";`

The workflow every claim in this section is about.

## `pub(super) const CLIPPY_GATE: &str = "cargo clippy --all-targets --all-features -- -D warnings";`

The command a platform's Clippy leg must run, character for character.

Character for character is the point. `- run: echo cargo clippy ...` is
`BRIDGE-CI-SHAPE-TEST-IS-A-SUBSTRING-ORACLE`'s first escape: it satisfies any
`contains` check while the job merely echoes, succeeds, and lets the
aggregate settle green over a platform Clippy never examined.

## `pub(super) const TEST_COMMAND: &str = "cargo test --all-targets --all-features";`

The command that executes this file's fixtures, character for character:
the whole suite, with no selection. No job runs it whole now. Every leg runs
it over a selection: the `test` job's as [`TEST_SHARD_COMMAND`], and the
Windows shards' on line 4 of [`WINDOWS_SHARD_WITNESS`], which starts `cargo`
as a process with this command's arguments and the leg's selection after
them. Until 2026-11-20 the guest's Windows job ran it whole, as one job
([`TEST_WINDOWS_HOSTED_JOB`]).

## `pub(super) const TEST_SHARD_COMMAND: &str =`

The `test` job's suite step, character for character: [`TEST_COMMAND`], then
` -- ${{ matrix.tests }}`. The expression is text here, as it is in
`ci.yml`; GitHub replaces it with the leg's `tests:` value from the job's
matrix ([`TEST_LEGS`]) before bash reads the line. What follows `--` goes to
libtest in every harness `--all-targets` builds.

- On every leg, Linux or macOS, the value is the shard's filter, if it has
  one, and a `--skip` for each filter of the legs before it on the same
  runner. Without the selection each of the seven legs runs its platform's
  whole suite.
- Until the Linux suite was divided its one leg's value was the empty
  string: the line ended at `--`, libtest was handed nothing, and the leg
  ran the whole suite, as it did when the step was [`TEST_COMMAND`] alone.

One step for every leg, so nothing in it varies by leg except through that
expression and the case-fold map's ([`TEST_STEP_ENV`]); what the expression
evaluates to is the matrix check's to hold. The oracle recognises the suite
step by this text, and
`the_workflow_that_runs_these_tests_installs_the_compiler_they_need` holds
it to [`TEST_COMMAND`] with the selection appended and nothing else, so the
two constants cannot drift apart with each pin holding. Measured,
`MUT-CI-CARGO-TEST-STEP-DELETED`, `MUT-TEST-SHARD-SELECTION-DROPPED` and
`MUT-TEST-FILTER-HARDCODED`.

## `pub(super) const WINDOWS_BUILD_WITNESS: &str = "cargo build --all-targets --all-features";`

The codegen and link witness on current stable for the platform whose tests
run in their own job, character for character.

`cargo check` and Clippy type-check and stop before codegen, and the
Windows suite's shards link with the pinned [`GOLDEN_IMAGE_TOOLCHAIN`],
which moves only by an edit to that pin. Without this step nothing on
GitHub's current stable ever
code-generates or links the Windows tree: a Windows-only codegen or link
failure on current stable would pass every other leg while the shards, on
the pinned compiler, link and pass. `cargo build --all-targets` rather than
`cargo test --no-run`: the latter builds only test-profile artifacts, so a
`#[cfg(all(windows, not(test)))]` body in a binary is never linked by it,
where `--all-targets` links the library and the binaries **and** their
unit-test harnesses, plus examples and integration tests. It executes
nothing; the suite's execution is the shards'.

In the dev profile. `cargo test` builds the test profile, which inherits
from dev and is identical to it until the manifest says otherwise, and that
compile happens in the shards with the pinned toolchain; so what is compiled
on current stable is the dev-profile compile and link of every target, and
what is compiled with the pin is the test-profile compile. A
`[profile.test]` override that splits the two is a manifest edit in the
diff. A link failure that needs `--release` is not covered here and is not
covered on `master` either; `release.yml` builds the shipped artifact with
`--release`, and that build failing is a red release, not a green CI over a
broken tree.

## `pub(super) const GIT_IDENTITY_SCRIPT: &str = "git config --global user.email \"ci@upstroke.local\"\ngit config --global user.name \"upstroke CI\"\n";`

The script the test jobs run before the suite, character for character:
an identity for the commits several tests make.

Pinned because a `run:` step is a shell with the checkout in front of it.
`git fetch origin master && git checkout --detach FETCH_HEAD` appended here
would test `master` while the runner, the fields, the shell, the input-free
checkout and the pinned test command that follows all still matched.
Measured, `MUT-TEST-WINDOWS-HOSTED-RUN-RETARGETED` and
`MUT-TEST-RUN-RETARGETED`.

## `pub(super) const TEST_SCRIPTS: [&str; 4] = [`

Every script the hosted test job may run: the identity script, the suite
over the leg's selection ([`TEST_SHARD_COMMAND`]),
[`HOST_ACCEPTANCE_CAPTURE`] and [`CAPABILITY_OBSERVATION`]. Each is one exact
string; the list grows by one pinned script at a time, and nothing in it is a
pattern.

## `pub(super) const TEST_WINDOWS_HOSTED_SCRIPTS: [&str; 4] = [`

The scripts every leg of [`TEST_WINDOWS_HOSTED_JOB`] runs: the identity
script, the suite with the witness that it executed
([`WINDOWS_SHARD_WITNESS`]), [`HOST_ACCEPTANCE_WINDOWS_CAPTURE`] and
[`CAPABILITY_WINDOWS_OBSERVATION`]. Each is one exact string, as in
[`TEST_SCRIPTS`].

The identity script is the `test` job's, the same on any runner. The two
acceptance-record steps were the guest's job's as well until 2026-11-20,
the same two constants, because neither depended on the guest's image: each
runs the test it names and checks the frame it prints, and every container
capability class passes, an absent Docker included. Each of the ten legs
runs both, after its own share of the suite, so a run prints ten Windows
host acceptance records and ten capability observations, where the guest's
one job printed one of each.

## `pub(super) const WINDOWS_TEST_FLOOR: u32 = 1700;`

The sum the ten shard floors must make ([`WINDOWS_SHARD_FLOORS`]): the floor
the whole Windows suite was held to as one job. No script reads it now. Each
leg is held to its own floor, and
`the_shard_tables_run_every_test_in_exactly_one_shard` holds the ten to this
sum, so the legs together accept no fewer passing tests than the one job
did.

At `cb8ac1f` the one job reported 1771: 1761 from the lib harness, 10 from
the bin one, 0 from the example. The floor sat below that by a margin wide
enough that ordinary churn did not touch it and narrow enough that a suite
which silently stopped running could not clear it. The ten legs counted
3,558 passing tests between them in run 44736631711, and each floor is about
0.48 of its shard's count ([`WINDOWS_SHARD_FLOORS`]). Dropping this many
Windows tests is a deliberate act, and it edits the shard floors and this
number in the same change.

## `pub(super) const WINDOWS_SHARD_WITNESS: &str`

The suite step of every leg of [`TEST_WINDOWS_HOSTED_JOB`], character for
character: a refusal to run on any compiler but the pinned one, then the
pinned test command over the leg's selection, started as a process and
waited on, its output read from files, the test processes it left running,
its exit status, a count of what libtest reported held to the leg's floor,
and the count printed. Eleven lines. The three `matrix` expressions are text
here, as they are in `ci.yml`; GitHub replaces each with the leg's value
from the table the job's matrix carries before PowerShell reads the script.

The compiler check comes first, on lines 1 and 2. Step 1 of the job
installs [`GOLDEN_IMAGE_TOOLCHAIN`], and a green install is not yet that
compiler running the suite: the toolchain action tolerates a failed `rustup
default` and then verifies only `rustc +1.97.1`, so it can report success
while bare `cargo` is still the runner's preinstalled stable. The questions
are what make the install the claim rather than a hope. And
the `rustc` on PATH is not what compiles: Cargo selects its compiler
through `RUSTC`, `CARGO_BUILD_RUSTC` or a `build.rustc` in any config it
reads, and the runner's environment and Cargo home are inputs this
contract cannot see. So the script asks Cargo itself --
`cargo rustc --lib -- --version` runs the compiler Cargo would run and
prints its version, and honours every one of those overrides. It compiles
the dependencies to answer, which the suite would compile next anyway. The
`rustc` on PATH is a second question rather than a substitute: the suite's
own fixtures spawn bare `rustc` against what Cargo built -- the effect-denial
fixtures here, the log and run-directory tests -- and a PATH rustc of
another version fails them with an incompatible-rlib error, so a leg
whose PATH disagrees with its Cargo is not the same suite either. The
script asks all three, Cargo's compiler, PATH's rustc and PATH's cargo,
and throws unless every one is [`GOLDEN_IMAGE_TOOLCHAIN`]'s.
`the_windows_leg_counts_the_tests_it_ran` holds the number in the script to
the constant, and
`the_hosted_windows_shards_run_the_suite_on_the_compiler_they_install` holds
the install to it, so moving the compiler edits the install, the script and
the constant together. The refusal's message still calls the version "the
image's": it was the guest's image's, and the words are kept with the pin.
Measured, `MUT-TEST-WINDOWS-HOSTED-COMPILER-CHECK-DROPPED`.

What the three questions bound is the step's own process: a rustup default
that did not move, a `RUSTC` in the runner's environment, a PATH that names
another install. The test executables get a process of their own, which
Cargo builds: it sets the platform's library-path variable on every one --
on Windows that is `PATH`, its output directories prepended -- and then
applies a Cargo config's `[env]` table, skipping any key it has already
set. That skip is a case-sensitive lookup and Windows' environment is not,
so on Windows `[env] PATH = { value = "...", force = true }` is ignored
and the same table spelled `Path` replaces the fixtures' PATH: all three
questions answer the pinned compiler and the fixtures spawn another
(frontier review 9, executed on the dev guest; Cargo's source at
`c980f4866`). The questions do not see that split. The suite does: four of
the five fixtures that spawn a compiler hand it this crate's rlib with
`--extern` -- `every_declared_effect_denial_refuses_for_the_reason_it_declares`
and `every_denied_path_this_host_can_resolve_does_resolve` here,
`every_declared_build_refusal_fails_for_the_reason_it_declares` in the log
tests, `a_private_half_deletion_without_a_proof_does_not_compile_for_the_stated_reason`
in the run-directory tests -- and `rustc` refuses an rlib another version
built. Under the split all four fail with E0514 and `cargo test` exits 101
(review 9, executed at `fb136de5` on Linux, where Cargo sets no `PATH` and
the plain spelling applies; on Windows the E0514 was executed in a probe
crate, and the suite itself was not run). The fifth,
`the_file_level_lint_reader_answers_what_rustc_does`, links nothing and
passes on the other compiler, but only in a run the other four have already
failed. So no green run of the leg that runs those four has spawned a
fixture compiler other than the one Cargo built with. By
[`WINDOWS_SHARD_FILTERS`] that is the last leg, the one with no filter: the
five fixtures' paths contain none of the nine. That bound is the fixtures',
not the questions', and it rests on those four tests running on a Windows
leg, which nothing here pins by name: the floors count tests, not which
ones.

Every other pin in this contract is an equality over `ci.yml`, and each
refuses one named way of arriving at a green job over a suite that never
ran. This one refuses the arrival itself. Cargo can be told to compile every
harness and execute none by a `target.<triple>.runner` in a repository
`.cargo/config.toml`, in `$CARGO_HOME`, in any directory above the checkout,
or in the process environment, and it can be told to build no harness of
this crate at all by a root `[workspace]` whose `default-members` name
another one. Three of those five are written where nothing reading this
repository can see them, the fifth has as many spellings as TOML has
whitespace, and the list closes only if Cargo stops growing. A count does
not enumerate them: a wrapper that executes nothing and prints nothing
leaves no `test result: ok.` line, whichever route installed it, and that
is every misconfiguration and every lazy substitution. It is not every
substitution. A wrapper written to print libtest's summary line clears the
floor, and so does a decoy crate whose `harness = false` main prints it;
both are a forgery, on the machine or in the diff, and the paragraph below
is where forgeries are bounded.

What it does not do is bound a hostile candidate, and no step in this file
does. An edit to `ci.yml` deletes this one as easily as the guards it
replaces, and the decision record says so where it says where the boundary
actually is. What it bounds is the machine. The count was written for the
guest, which was provisioned outside the repository, so that its Cargo home
and its environment were inputs no reviewer of a diff could check. It stays
now that the suite runs on GitHub's runners, because a Cargo target runner
or a `[workspace]` selection reaches them the same way, and this is each leg
saying that it ran what it claims to have run. A leg's floor also refuses
what the whole suite's could not see: a shard whose filter matched almost
nothing.
Measured, `MUT-TEST-WINDOWS-HOSTED-FLOOR-DROPPED` and
`MUT-TEST-WINDOWS-HOSTED-WITNESS-DROPPED`.

Between the compiler check and the count is the suite itself, lines 3 to 8,
started as a process and waited on:

3. Two paths in `RUNNER_TEMP`, one for each of Cargo's streams.
4. `Start-Process` starts `cargo` with [`TEST_COMMAND`]'s arguments and,
   after them inside the one quoted argument list, ` -- ${{ matrix.tests }}`.
   Its streams are sent to those files, and it hands back the process
   (`-PassThru`). What follows `--` goes to libtest in every harness
   `--all-targets` builds: the leg's filter, if it has one, and a `--skip`
   for each filter of the legs before it. Without the selection every leg
   runs the whole suite.
   `$null = $suite.Handle` has the process object open its handle while
   Cargo runs, without which `ExitCode` can read empty once the process has
   ended. `WaitForExit()` returns when Cargo's own process ends, and `$code`
   keeps its exit. There is no `-Wait`, which waits for the process's
   descendants as well.
5. One `try` block, which reads and then lists. It prints the two files
   and keeps standard output as `$log` for the count; the job log carries
   the suite's output once Cargo has ended, not as it runs. It then lists
   every process named `upstroke*` that is still running, with its command
   line, and asks Windows to stop each one; the test harnesses are
   `upstroke-<hash>.exe`. `Stop-Process` is given
   `-ErrorAction SilentlyContinue` and nothing checks that the process
   stopped, so the stop is attempted, not ensured. The listing names a
   process, not the test that started it. A process left running under
   another name is neither listed nor asked to stop.
6. The `catch` for line 5. GitHub runs a `pwsh` step with
   `$ErrorActionPreference = 'stop'`, so a failed read or a failed listing
   ends line 5 where it fails. Where Cargo's exit was not zero, the `catch`
   prints that failure and the script goes on to line 7, so the failure
   cannot replace Cargo's exit code. Where Cargo exited zero, it throws the
   failure again and the step fails: nothing was read or listed, so nothing
   can be counted.
7. Cargo's exit, where it is not zero. The line rests on line 4 having kept
   one, as line 6's test does: PowerShell compares an empty value as not
   equal to zero, and `exit` given nothing exits zero, before the count.
8. A throw where line 5 listed anything.

What is said here of `Handle`, `ExitCode`, `-Wait`, `exit`, `try` and
`catch` was not executed where it was written: no PowerShell runs on the
build box ([`HOST_ACCEPTANCE_WINDOWS_CAPTURE`]). The legs' own runs are the
evidence for it.

Before this the suite was a pipeline: line 3 was [`TEST_COMMAND`] over the
leg's selection, piped into `Tee-Object -Variable log`, and line 4 read
`$LASTEXITCODE`. Twice a shard running that text reported its tests and did
not end:

- run 43719719561, 2026-11-15: the 1,669 s shard printed every
  `test result: ok` line 31.6 minutes into its job and ran to the job's
  limit, where the runner's clean-up killed a test process left over;
- run 44277078005, 2026-11-18: `test (windows-latest, 3 of 5)`, job
  134707868382, printed `test result: ok. 700 passed; 0 failed` at 09:50:41
  and ran until its 40-minute limit cancelled it, and the job's clean-up
  killed an orphaned test binary, `upstroke-8ddcc7d640fdf222`.

That is what was observed: the passing result line, the hang, and a test
binary killed at clean-up. The cause is inferred from it, and from Rust's
`Command`, which on Windows hands a child the handles its parent holds as
inheritable: a test process that outlived the suite held the pipe
`Tee-Object` reads, so the pipeline never reached its end. Which test
leaves the process is not established. Both runs were this job's. The
step is written against the inference. Waiting on Cargo's process does not
depend on what that process left running, and nothing the script reads is a
pipe a survivor can hold open; whether a leg that leaves a process now ends
is the evidence of the next run in which one does.

What is left running is the step's to report, where it was the runner's to
kill at clean-up. Line 5 runs after a failed suite as after a passed one, so
the processes are listed, and Windows is asked to stop them, either way.
Line 6 keeps a failure in line 5 from replacing Cargo's exit code, line 7
gives Cargo's failure that code, and line 8 fails a suite that passed and
left a process: the job that ran the test names the process's command line,
not the test. If the inference is wrong and Cargo itself does not end, line
5 is never reached, and the job's limit still ends the leg.
`the_windows_leg_counts_the_tests_it_ran` holds the script to line 4 as
[`TEST_COMMAND`]'s arguments and the selection make it, and to six things
about the process it starts: that it waits with `WaitForExit()` and not
`-Wait`, pipes nothing into `Tee-Object`, exits with Cargo's code before it
counts, lists the processes before that exit and throws for them after it,
opens with lines 1 and 2 whole and holds `cargo` after them once, on the
line that starts the process, and reads and lists inside the `try` whose
`catch` keeps Cargo's code. The six are `windows_suite_wait_complaints` in
the test notes.
Measured in the oracle, each an edit to the workflow that leaves this text
alone: `MUT-TEST-WINDOWS-HOSTED-SUITE-PIPED-AGAIN`, line 4 a pipeline into
`Tee-Object` again, and `MUT-TEST-WINDOWS-HOSTED-SURVIVOR-ALLOWED`, line 8
removed.
Measured against this text itself, each an edit a pin cannot refuse because
the workflow and the constant move together: the suite run a second time,
and the `try` and its `catch` removed
(`the_windows_suite_checks_refuse_a_second_run_and_a_masked_exit` in the
test notes).

The fifth of the six is the one that refuses a second run of the suite, and
it reads this text's letters. It is drawn unless all of these hold:

1. Line 1 is the compiler-questions line, whole.
2. Line 2 is their refusal, whole, built from [`GOLDEN_IMAGE_TOOLCHAIN`]
   with its dots escaped as line 2 writes them.
3. Each later line holds `cargo`, in any letter case, once if it holds
   `Start-Process` and never otherwise. The words
   `Cargo compiled the harnesses`, in line 10's message, are set aside
   before the count.
4. `Start-Process` occurs once.

Before `PR389-INDIRECT-SECOND-SUITE-RUN-UNREFUSED` the count skipped every
`cargo` that followed `$` or `^`, on any line. Round two's frontier pass
over #389 appended
`& (($cargo -split ' ')[0]) test --all-targets --all-features` as a last
line of the guest's script, in its constant and in `ci.yml` together, and
every `effects::` test passed over a script that runs the suite twice. No
`cargo` after line 2 is skipped now, whatever precedes it.

The rule is for an edit made in good faith, a second run a change leaves in
by mistake. It reads letters and parses nothing, so a second run the text
never spells still passes it:
`& ('car' + 'go') test --all-targets --all-features`, appended here and in
`ci.yml` together. That is read from the text, not run, and it is filed as
`PR389-UNSPELLED-SECOND-SUITE-RUN-UNREFUSED`, deferred. An edit to this
constant or to the workflow is the owner's to read. The rule does not bound
one, as no pin here bounds a hostile candidate (above).

After the suite, lines 9 to 11 are the count:

9. The sum of every `test result: ok.` line's passed count.
10. The refusal of a sum below `${{ matrix.floor }}`. The message names the
    shard and its floor.
11. The sum, the shard and the floor, printed. A leg that passes then says
    in its log how far above its floor it ran, which is what a later change
    to a floor is measured from.

The floor is not in this text. It reaches the script from the matrix, so
the equality over the script holds that line 10 reads `matrix.floor` and
the oracle's check of the matrix holds what each leg's floor is
([`WINDOWS_SHARD_FLOORS`]); a lowered floor in the table leaves this pin
intact and is refused there. The same division holds the selection: this
text says line 4 reads `matrix.tests`, and the matrix check says what each
leg's is ([`WINDOWS_SHARD_FILTERS`]).
`the_windows_leg_counts_the_tests_it_ran` holds line 10 to opening with the
test of `matrix.floor`, so the floors the oracle holds the matrix to are
floors a leg tests. Line 11 is held by the oracle's pin alone.

Measured, for the selection,
`MUT-TEST-WINDOWS-HOSTED-SHARD-SELECTION-DROPPED` and
`MUT-TEST-WINDOWS-HOSTED-FILTER-HARDCODED`. Each of the seven rows this
section names edits the step in the workflow alone, and each is refused
under `test-windows-hosted-command`.

The step was the guest's first. Until 2026-11-20 this record stood under
`WINDOWS_TEST_WITNESS`, the suite step of the guest's one job: ten lines,
these without the eleventh, with no selection on line 4 and with 1700 where
line 10 reads `matrix.floor`. The hosted job ran that text too, character
for character, until it ran the suite in shards. The constant went with the
guest's job ([`TEST_WINDOWS_HOSTED_JOB`]).

## `pub(super) const HOST_ACCEPTANCE_CAPTURE: &str = r#"set -o pipefail`

The hosted `test` job's capture of the native HOST acceptance record (G7,
root direction `087fea89`), character for character. It runs on every leg in
GitHub's default `bash -e` and must stand immediately after the suite step;
each of the five macOS legs and the two Linux legs runs it after its own
share of the suite, so a run prints five macOS records and two Linux ones
where it printed one of each.
Line by line:

1. `set -o pipefail`. Under `bash -e` a pipeline takes its status from its
   last command, `tee`, which succeeds; with `pipefail` it takes Cargo's.
2. The log goes to `$RUNNER_TEMP`, outside the checkout.
3. `cargo test --all-features --lib engine::tests::host_acceptance_record --
   --ignored --exact --nocapture`, through `tee`, so the record reaches the
   job log and the file the next lines read.
   - `--lib` selects the one harness the test lives in, which the suite step
     has just built.
   - `--exact` selects the one test.
   - `--ignored` runs it. Without that flag libtest reports it ignored and
     Cargo exits zero.
4. The named test's own pass line, and `test result: ok. 1 passed; 0 failed;
   0 ignored;`, or exit 1. A filter that matched nothing (`running 0 tests`),
   a test reported ignored, or a run of anything else is not a record.
5. An `awk` over the framed lines refuses a missing or incomplete frame. It
   requires BEGIN first and END last, with the same tag, count, length and
   digest; chunk `k` at position `k`, carrying the tag; every chunk's hex at
   its exact length; and the declared count equal to the lines found. All
   comparisons are strings, so no tag or digest is read as a number. It does
   not decode the hex or check the digest: that is the reader's job when it
   accepts the record, and a printed frame is never acceptance by itself.

Measured locally (`g7-host-acceptance-author-1/controls/ci-step-2`) under
gawk, `gawk --posix` and mawk: the unchanged capture passes; Cargo's 101
propagates; and a missing, truncated, wrapped, reordered, doubled or
mistagged frame, zero tests, an ignored test and two passing tests are each
refused. macOS's awk (BWK) was not available there. The script uses only
POSIX awk, and its run on macOS is that leg's evidence, not a local one.
Measured in the oracle: `MUT-TEST-CAPTURE-STEP-DELETED`,
`MUT-TEST-CAPTURE-IGNORED-NOT-RUN`, `MUT-TEST-CAPTURE-FILTER-MOVED`,
`MUT-TEST-CAPTURE-EXIT-SWALLOWED`, `MUT-TEST-CAPTURE-ZERO-TESTS-ADMITTED`,
`MUT-TEST-CAPTURE-FRAMING-UNCHECKED` and
`MUT-TEST-CAPTURE-SEPARATED-FROM-THE-SUITE`.

## `pub(super) const HOST_ACCEPTANCE_WINDOWS_CAPTURE: &str = r#"cargo test --all-features --lib engine::tests::host_acceptance_record -- --ignored --exact --nocapture | Tee-Object -Variable capture`

The same capture on every leg of [`TEST_WINDOWS_HOSTED_JOB`], under the
job's PowerShell, character for character. It stands immediately after the
suite-and-witness step, so it runs the harness that step built on the
compiler that step asserted.

- `$LASTEXITCODE` carries Cargo's exit through `Tee-Object`. The suite step
  read Cargo's exit the same way until it stopped piping Cargo
  ([`WINDOWS_SHARD_WITNESS`]); this step runs one test and still pipes.
- `-ccontains` requires the named test's pass line and `-cmatch` requires
  `1 passed; 0 failed; 0 ignored`.
- The framed lines, split on single spaces, must be BEGIN first and END last
  with the same summary, chunk `k` at position `k` with the tag, every chunk
  at its exact length, and the count equal to the lines found.

The last line is the one that throws. A frame check that is computed and not
acted on refuses nothing.

No PowerShell runs on the build box, so this script's execution is the
shards' evidence, on every run, and not a local one. Measured in the oracle:
`MUT-TEST-WINDOWS-HOSTED-CAPTURE-STEP-DELETED`,
`MUT-TEST-WINDOWS-HOSTED-CAPTURE-EXIT-SWALLOWED` (the `$LASTEXITCODE` line
removed), `MUT-TEST-WINDOWS-HOSTED-CAPTURE-SEPARATED-FROM-THE-WITNESS`, and
the step's case-fold map by `MUT-TEST-WINDOWS-HOSTED-CAPTURE-CASEFOLD-DROPPED`.
The rows that edited this script's own lines mutated the guest's job and went
with it on 2026-11-20. Since 2026-11-21 the first two rows above measure the
step deleted and its exit line removed on the shards. Any other edit makes
the capture an unpinned script, the refusal
`MUT-TEST-WINDOWS-HOSTED-RUN-RETARGETED` measures on the identity step, and
those edits are measured on the `test` job's capture
([`HOST_ACCEPTANCE_CAPTURE`]).

## `pub(super) const CAPABILITY_OBSERVATION: &str = r#"set -eo pipefail`

The hosted `test` job's container capability observation (G7, root direction
`ecf15a9c`, contract version 2 under its root audit 2), character for
character. It runs on every leg in GitHub's default `bash -e`, the three
macOS legs each, and must stand immediately after
[`HOST_ACCEPTANCE_CAPTURE`]. Line by line:

1. `set -eo pipefail`. Under `bash -e` a pipeline takes its status from `tee`;
   with `pipefail` it takes Cargo's. It is spelled with `-e` because the bare
   `set -o pipefail` line is the anchor `MUT-TEST-CAPTURE-EXIT-SWALLOWED`
   requires once in this job.
2. The log goes to `$RUNNER_TEMP`, outside the checkout.
3. `cargo test --all-features --lib
   runner::container::tests::container_capability_record -- --ignored --exact
   --nocapture`, through `tee`. This is the brief's command exactly. `--lib`
   selects the harness the suite step built, `--exact` the one test, and
   `--ignored` runs it.
4. The named test's own pass line, and `test result: ok. 1 passed; 0 failed;
   0 ignored;`, or exit 1. A filter that matched nothing, an ignored test or a
   run of anything else is not an observation.
5. The PR355 `awk` frame check on this record's marker refuses a missing or
   incomplete frame: BEGIN first and END last with the same summary, chunk `k`
   at position `k` with the tag and its exact length, and the count equal to
   the lines found. Decoding and the record's own checks are the reader's job,
   and a printed frame is never acceptance.

Every capability class passes the test, an absent Docker included, so this
step fails only on what the contract calls observer integrity:

- an unresolved process;
- a binding failure;
- a record its own decoder refuses;
- a framing failure;
- Cargo's own failure.

Measured locally (`g7-capability-observer-author-1/controls/ci-step-1`) under
the awks the build box has. Measured in the oracle:

- `MUT-TEST-CAPABILITY-STEP-DELETED` (omitted);
- `MUT-TEST-CAPABILITY-IGNORED-NOT-RUN` and `MUT-TEST-CAPABILITY-ECHOED`
  (no-op);
- `MUT-TEST-CAPABILITY-FILTER-MOVED` and `MUT-TEST-CAPABILITY-FILTER-WIDENED`
  (filtered);
- `MUT-TEST-CAPABILITY-EXIT-SWALLOWED`,
  `MUT-TEST-CAPABILITY-ZERO-TESTS-ADMITTED` and
  `MUT-TEST-CAPABILITY-FRAMING-UNCHECKED`;
- `MUT-TEST-CAPABILITY-SEPARATED-FROM-THE-CAPTURE` and
  `MUT-TEST-CAPABILITY-ABOVE-THE-CAPTURE` (misplaced).

## `pub(super) const CAPABILITY_WINDOWS_OBSERVATION: &str = r#"cargo test --all-features --lib runner::container::tests::container_capability_record -- --ignored --exact --nocapture | Tee-Object -Variable capture`

The same observation on every leg of [`TEST_WINDOWS_HOSTED_JOB`], under the
job's PowerShell, character for character. It stands immediately after
[`HOST_ACCEPTANCE_WINDOWS_CAPTURE`].

- `$LASTEXITCODE` carries Cargo's exit through `Tee-Object`.
- `-ccontains` requires the named test's pass line, and `-cmatch` requires
  `1 passed; 0 failed; 0 ignored`.
- The framed lines must be BEGIN first and END last with the same summary,
  chunk `k` at position `k` with the tag, every chunk at its exact length, and
  the count equal to the lines found.
- The last line is the one that throws.

No PowerShell runs on the build box, so this script's execution is the
shards' evidence, on every run, and not a local one. Measured in the oracle:
`MUT-TEST-WINDOWS-HOSTED-CAPABILITY-STEP-DELETED`, and the step's bound and
case-fold map by `MUT-TEST-WINDOWS-HOSTED-CAPABILITY-TIMEOUT-DROPPED` and
`MUT-TEST-WINDOWS-HOSTED-CAPABILITY-CASEFOLD-DROPPED`. The rows that edited
this script's own lines, or put a step between it and the capture, mutated
the guest's job and went with it on 2026-11-20. On the shards an edited
observation is an unpinned script, as an edited capture is
([`HOST_ACCEPTANCE_WINDOWS_CAPTURE`]), and the edits themselves are measured
on the `test` job's observation ([`CAPABILITY_OBSERVATION`]).

## `pub(super) const CAPABILITY_STEP_FIELDS: [&str; 4] = ["env", "name", "run", "timeout-minutes"];`

The observation step's exact field set, on both jobs that run it, `test`
and `test-windows-hosted`, every field required.

- It has no `if:` and no `continue-on-error:`: the contract places it
  success-only, so a failed suite or capture stops it before it runs.
- It has no `shell:`, `uses:` or `with:`.
- `timeout-minutes` is the one field it carries beyond what the suite's step
  may, and only this step carries it; [`TEST_STEP_FIELDS`] and
  [`STEP_FIELDS`] are unchanged.

Measured, in the `test` job, `MUT-TEST-CAPABILITY-STEP-DISABLED`,
`MUT-TEST-CAPABILITY-STEP-RUN-WHEN-NOT-CANCELLED` and
`MUT-TEST-CAPABILITY-STEP-ABSOLVED`, each a field the set leaves out. On the
shards, `MUT-TEST-WINDOWS-HOSTED-CAPABILITY-TIMEOUT-DROPPED`, a required
field gone.

## `pub(super) const CAPABILITY_STEP_TIMEOUT_MINUTES: i64 = 4;`

The step's bound. The questions request at most 135 s (10 + 60 + 20 +
3 x 15), and Cargo's start comes on top. That sum is the requested bounds,
not a proved wall-clock maximum, and the four minutes are an operational
limit, not a guarantee that every future job fits. The tightest leg is a
`test` leg, at 30 minutes ([`TEST_TIMEOUT_MINUTES`]); until 2026-11-20 it
was the guest's 20-minute job. Measured, `MUT-TEST-CAPABILITY-TIMEOUT-DROPPED`,
`MUT-TEST-CAPABILITY-TIMEOUT-WIDENED` and
`MUT-TEST-WINDOWS-HOSTED-CAPABILITY-TIMEOUT-DROPPED`.

## `pub(super) const FMT_GATE: &str = "cargo fmt --check";`

The formatter gate and the six shell gates the `lint` job runs from the
repository root, character for character.

## `pub(super) const GATE_SCRIPTS: [&str; 8] = [`

Every script a gate job may run. A union across the three gate jobs rather
than a set per job: `lint (macos)` running the formatter would be
redundant, not an escape, and the one command that must sit on exactly one
job, the Windows build witness, is pinned to its carrier separately.

## `pub(super) const PINNED_ACTIONS: [&str; 4] = [`

The actions a step may `uses:`, each pinned to the commit it was reviewed
at. An action off this list is code nobody here reviewed running with a
checkout of its own; a floating tag (`@v4`) is that action at whatever
commit the tag points to tomorrow. Measured, `MUT-STEP-USES-UNPINNED`.

`actions/setup-python` is here for one step: `lint` puts CPython 3.12 on
PATH before `test-pr-ready-audit.sh`, whose decode probe asserts answers
measured on 3.12 and refuses any other interpreter.

## `pub(super) const ACTION_INPUTS: [(&str, &[&str]); 4] = [`

The inputs each pinned action may be given, by action prefix.

Pinning the commit says which code runs; this says what it is told to do,
and the difference is not academic. `Swatinem/rust-cache` accepts a
`cmd-format` input and wraps the commands it runs in it, so a cache step
with one input can run `git checkout` against another tree before Clippy and
the build witness ever see the candidate -- with the action allowlisted, the
checkout input-free, and every `run:` still exact. An input this contract
does not model is refused for the same reason an unknown field is.
Measured, `MUT-CACHE-CMD-FORMAT`.

`actions/setup-python` is given `python-version` alone, which says which
interpreter it puts on PATH; whether that one is acceptable is the gate's
to decide, and it accepts CPython 3.12 only.

## `pub(super) const TOOLCHAIN_COMPONENTS: [&str; 2] = ["clippy", "rustfmt, clippy"];`

The `components` values the toolchain action may be given.

The values, not just the key. The action builds a shell command from this
input and interpolates it into a later Bash line, so a value carrying
`;git fetch origin master && git checkout --detach FETCH_HEAD;` installs
Clippy, checks out another tree, and exits zero — through an allowlisted
action at a pinned commit with an allowlisted input name. Measured,
`MUT-COMPONENTS-INJECTED`.

## `pub(super) const GOLDEN_IMAGE_TOOLCHAIN: &str = "1.97.1";`

The compiler the Windows suite runs on: the version the guest's golden image
carried when the guest was retired on 2026-11-20, kept as the pin. Each leg
of [`TEST_WINDOWS_HOSTED_JOB`] installs it, exactly, at step 1, and lines 1
and 2 of [`WINDOWS_SHARD_WITNESS`] refuse to run the suite on any other. Not
`stable`, which is ahead of it (`lint (windows)` installed 1.98.1 on
2026-09-16 against the image's 1.97.1).

The pin is kept so that the suite's compiler moves by an edit -- the
install, the two lines of the suite step and this constant, together -- and
not with GitHub's stable. Under `stable` a test whose outcome depends on the
compiler -- a version-gated test, a std change to Windows process spawning,
a new lint in test code under `-D warnings` -- passes one week and fails the
next on an unchanged tree. `lint (windows)` compiles, code-generates and
links the tree on current stable on every event and is required by the
aggregate, so a stable that adds a lint between a pull request's green and
its enqueue ejects the entry through that leg; nothing here shields the
queue from that.

The constant's name is the guest's, and so are the install step's name,
`Install the image's toolchain`, and the words "the image's" in the suite
step's refusal. All three are kept. No image carries this compiler now: the
version is the one the image's was.

While the guest ran the suite, its image selected the compiler and
re-curation moved it, editing this constant in the same change. The guest's
job installed nothing, and an install there was refused outright, since at
this very version it would have passed the witness while the workflow, not
the image, chose the compiler. While the merge queue ran on `windows-latest`
its lane installed this version, so that the queue and the pull-request lane
ran one compiler, and that install went with the lane. The hosted job, added
beside the guest's, had no image, so it installed this version too. It first
installed `stable`; the first review of #375 found that this brought back
the compiler split the hosted queue lane's pin had removed, and the operator
pinned the job to the guest's compiler on 2026-11-12, so that what differed
between the suite's two homes was the machine and not the compiler.

Measured, `MUT-TEST-WINDOWS-HOSTED-TOOLCHAIN-FLOATS`,
`MUT-TEST-WINDOWS-HOSTED-TOOLCHAIN-DOWNGRADED` and
`MUT-TEST-WINDOWS-HOSTED-COMPILER-CHECK-DROPPED`.

## `pub(super) const TEST_WINDOWS_PLATFORM: &str = "windows-latest";`

The [`CI_TARGETS`] runner whose tests run in [`TEST_WINDOWS_HOSTED_JOB`], on
this very runner, rather than in the `test` matrix. The job's steps are read
under its shell and cfg valuations: a `run:` step there resolves to `pwsh`,
and it builds the MSVC tuple. The oracle holds the job's `runs-on:` to this
runner and the `test` job's table to every other. Measured,
`MUT-TEST-WINDOWS-HOSTED-ON-THE-GUEST` and `MUT-TEST-MATRIX-KEEPS-WINDOWS`.

## `pub(super) const TEST_WINDOWS_HOSTED_JOB: &str = "test-windows-hosted";`

The job that runs the Windows suite, on every event: a pull request, a push
to `master` and a merge-queue entry alike. It is a matrix of ten legs on
GitHub's `windows-latest`, each on a runner of its own. Each leg installs
[`GOLDEN_IMAGE_TOOLCHAIN`] with `clippy` at step 1 and then runs
[`TEST_WINDOWS_HOSTED_SCRIPTS`]: one shard of the suite, on the pinned
compiler, against that shard's share of 1700, the floor the whole suite was
held to as one job. The job declares no `needs:` and no `if:`, and no other
job runs the suite.
`the_hosted_windows_shards_run_the_suite_on_the_compiler_they_install`
holds the job to the oracle and its install to the version the suite step
asserts. `hosted` in the job's id is from when it was the second of the
suite's two homes (History, below).

The name is `test (windows-latest, ${{ matrix.shard }})`, an expression, and
the constant holds it as the workflow writes it. GitHub lists each leg under
the name with its shard in place, `test (windows-latest, 1 of 10)` to
`test (windows-latest, 10 of 10)`. The oracle holds the `name:` to the
constant, and nothing reads the legs' listed names: until 2026-11-20 they
were the names another run's picker was handed, and it read none of them.

Shards, because the suite as one job stopped fitting. Measured on
2026-11-15, run 43719719561, on `windows-latest` with four processors: the
whole suite took 3,311 to 4,292 s in the harness, 59 to 76 minutes a job,
past the 55 minutes this job then ran under. Four shards by test name took
692 to 1,669 s and ran 193 + 776 + 320 + 2,218 = 3,507 tests, the whole
suite's count; the slowest leg that ended took 22m 53s. The 1,669 s shard,
the rest of `engine::topology::`, was divided in two, which made five. Draft
pull request #339 had proposed sharding the suite.

Ten legs since 2026-11-19, divided by measured time, because a run's slowest
leg sets its wall clock. In run 44614925593 the five legs' libtest harnesses
took 1201, 508, 976, 564 and 860 s, and the slowest job took 24.3 minutes,
against about 3.6 minutes fixed per leg: setup, the compiler questions, a
1m45s to 2m46s compile, and the two record captures. The nine filters of
that day came from per-test weights of a sequential Linux run, scaled to
those five walls and blended with a uniform per-test model, and the slowest
of the ten legs was predicted at about 13 to 16 minutes.

Run 44667503206 measured those ten for the first time. The jobs took 6.4,
8.1, 15.8, 12.1, 14.0, 10.0, 18.6, 4.5, 13.2 and 7.4 minutes, and their
libtest harnesses 189, 209, 708, 498, 636, 368, 878, 76, 545 and 223 s,
4330 s in all, with about 3.8 minutes of each job fixed. The slowest, at
18.6 minutes against the 13 to 16 predicted, was the seventh, the rest of
`engine::`.

The filters of [`WINDOWS_SHARD_FILTERS`] are the re-split of 2026-11-20,
made from that run. Each test's sequential Linux duration is scaled so that
the leg it ran in sums to that leg's measured harness, and blended half and
half with a uniform per-test model; the filters were then chosen by a
search for the slowest leg. The slowest of the ten was predicted at about
14.4 minutes. Run 44736631711 measured the re-split for the first time: its
slowest leg, the fifth, took 18.1 minutes.

Single tests set the floor under that prediction. A leg takes no less than
its longest test and the fixed minutes, and two tests are each estimated at
about 9 minutes of their leg's harness:

- `engine::topology::recover::tests::a_kill_at_every_cell_of_a_complete_finalization_converges_on_the_next_resume`;
- `workspace_manager::tests::r_o2_a_cross_kind_scrub_stopped_at_any_phase_converges_with_no_instance_registered`.

The re-split changes neither test.

The aggregate reads the ten legs through one result,
`needs.test-windows-hosted.result`, as it reads the `test` and `msrv`
matrices through one result each, and it accepts `success` alone from it, as
from every gate ([`AGGREGATE_SCRIPT`]). `fail-fast` is `false`, so a failed
leg does not cancel the others before they report. What GitHub gives a
matrix whose leg did not finish is observed, not a rule stated here: a leg
GitHub cancelled without a runner made its matrix `abandoned` and the
aggregate red, in runs 44114671287 (`msrv`) and 44149091955 (`test`). No
hosted shard has been lost that way yet. A `success` reported over an
unfinished leg is what the rule cannot refuse.

40 minutes a leg: 1.75 times the slowest leg that ended in run 43719719561,
and inside the merge queue's 60. In that run the 1,669 s shard printed every
`test result: ok` line 31.6 minutes into its job and did not end. It ran to
the job's limit,
where the runner's clean-up killed a test process left over; that the
process held the step's output until then is inferred, not seen. The limit
bounds the hang and does not repair it. It happened again on
2026-11-18, in run 44277078005, and the suite step now waits on Cargo's
process where it piped Cargo; what the two runs showed, and what is inferred
from it, is under [`WINDOWS_SHARD_WITNESS`].

The job is recognised by its id, runs on [`TEST_WINDOWS_PLATFORM`] as a
scalar, and carries no condition. Measured,
`MUT-TEST-WINDOWS-HOSTED-RENAMED-AWAY`,
`MUT-TEST-WINDOWS-HOSTED-ON-THE-GUEST`,
`MUT-TEST-WINDOWS-HOSTED-CONDITIONED` and
`MUT-TEST-WINDOWS-HOSTED-TIMEOUT-RAISED`; the matrix's own rows are under
`windows_shard_matrix_complaints` in the workflow notes.

**History, September to November 2026.** The Windows suite is spawn- and
worktree-heavy, and on `windows-latest` it was the whole CI wall clock: a
median of 12.5 minutes and a tail of 21 on identical code, because the
harness varied 535-1154 s with the host the runner landed on. On 2026-09-01
it moved to an ephemeral self-hosted guest on the build box -- a throwaway
overlay of a frozen image, one job per boot, registered with a single-use
just-in-time config -- and a second guest took the merge queue's builds so a
queue entry never waited behind a pull request. Measured again on
2026-09-15/16: five hosted samples (run 34980597250, at `126b8832`) ran the
harness in 1122-1499 s, 21m 30s to 28m 03s a job, and the guest, on three
green runs of pull request #299, in 346 s, 6m 47s to 6m 55s a job -- three
to four times faster.

From 2026-09-16 the queue lane ran on `windows-latest` instead, so that the
second guest could be retired from a box where two guests and the builds
contended for one set of cores, under a 45-minute timeout and then a
55-minute one. In October 2026 three queue entries of one head reached the
45 minutes and two entries of the next head the 55, each cancelled before
its suite reported a complete result, while the guest ran the full suite of
the tree the second of those two entries tested, as a pull request, inside
its own 20-minute timeout. The queue lane then returned to the guest -- the
same labels and the same guest as every other build, not the retired second
one -- as a scheduling repair that named no cause for the hosted runs and
repaired no test. One guest then served every lane, and a queue entry
waited for it behind whatever build held it.

The guest was one machine on the build box, shared with the operator's
baselines and Upstroke runs; on a busy day each made the other fail, and the
owner asked on 2026-11-11 for a second home used only then. From 2026-11-12
a `pick-windows-home` job answered `guest` or `hosted` for each run, and
this job ran the suite for the second answer, as the guest's hosted twin.
Each of the two suite jobs needed the picker and ran under a condition on
its answer, and the aggregate required exactly one of them to succeed and
the other to be skipped. Leaving the guest was opt-in, through three
repository variables: `WINGUEST_BUSY`; a `WINGUEST_QUEUE_LIMIT` the guest's
queue had reached, which the picker counted in the other runs of the
workflow through the API; and, later, `WINGUEST_PR_HOSTED`, for a pull
request's runs alone. With none set every run stayed on the guest, and a
picker that could not read the API answered `guest`.

The picker answered for each run on its own. So a change could pass as a
pull request and as a queue entry on one home, and its push to `master` be
its first run on the other, where a test whose outcome depends on the
machine was then first seen. While there were two homes, `MAINTAINING.md`
step 3 asked for a Windows-only failure to be run on the other home before
it was believed.

With `WINGUEST_BUSY` set on 2026-11-19, every run came here. On 2026-11-20
the guests were deleted from the build box on the owner's direction. The
picker, the guest's job, `test-windows`, which GitHub listed as
`test (winguest)`, and the reads of the three variables were removed from
the workflow, and their constants, checks, ledger rows and fixtures from
this contract. What was measured on the guest stays in these notes as what
was measured there.

## `pub(super) const CACHE_ACTION: &str = "Swatinem/rust-cache@";`

The cache action, by prefix, which [`TEST_WINDOWS_HOSTED_JOB`] may not use.
The job carries no cache step, and the oracle refuses one at any position.

The reason its cache step was taken out has gone. A run restores caches from
its own ref or the default branch. A merge-queue entry's ref is a fresh
`gh-readonly-queue/` branch, and until 2026-11-20 `master` saved a cache for
this job only on a push the picker itself sent here, which with routing
opt-in was rare; so nearly every run would have started cold and then
uploaded, inside its 40 minutes, a cache nothing later read, spending the
repository's cache space. That was the hosted merge-queue lane's reason for
carrying none before `d3434ace`. The job first carried one, "like the other
hosted jobs", which the first review of #375 found did not answer that
reason; the operator removed it on 2026-11-12. A pull request sent here
twice was the one run that would have gained.

Every push to `master` now runs these legs, so `master` would save a cache
that pull requests and queue entries could restore. Whether that pays inside
a leg's 40 minutes is not measured, and adding a cache step is a change of
its own, made together with this refusal. Measured,
`MUT-TEST-WINDOWS-HOSTED-CACHE-RESTORED`.

## `pub(super) const WINDOWS_SHARD_FILTERS: [&str; 9] = [`

The ordered filters that divide the Windows suite into ten shards, as data:
the list the matrix of [`TEST_WINDOWS_HOSTED_JOB`] is held to. The table
came first, with the proof that it
loses no test and repeats none, and the matrix is derived from it rather
than written beside it: `windows_shard_matrix_complaints` in
`super::workflow` requires each leg's `tests:` in `ci.yml` to be the string
`shard_selections` makes of this list for that shard, in order. The ten
strings are written out in one test of the oracle, the one named below. The
second is written out once more, in the anchor of the ledger row that
deletes that leg, `MUT-TEST-WINDOWS-HOSTED-SHARD-DROPPED`; the other rows
are under `windows_shard_matrix_complaints` in the workflow notes.

The rule is `shard_selections`' in `super::workflow`. Shard `k` runs the tests
whose names contain filter `k` and none of the filters before it, and one
more shard, after the last filter's, runs the tests that contain none of
them. Nine filters make ten shards:

1. `engine::topology::coordinator::tests::a_p`: everything in the
   coordinator's `tests` module whose name begins with `a_p`;
2. `engine::topology::coordinator::tests::a_r`: everything there whose name
   begins with `a_r`;
3. the rest of `engine::topology::coordinator::tests::a_`, the names there
   that begin with `a_`;
4. the rest of `engine::topology::coordinator::`, the names in its `tests`
   module that begin with `a` and not with `a_` among them;
5. `engine::topology::recover::`;
6. `engine::tests::a`: everything in the engine's own `tests` module whose
   name begins with `a`;
7. `runner::`;
8. `workspace_manager::`;
9. the rest of `engine::tests::`;
10. everything else: the rest of `engine::topology::`, the rest of
    `engine::`, which no filter names, and the bin harness's tests among
    them.

The list was four filters and five shards until 2026-11-19:
`engine::topology::coordinator::tests::a`, the rest of
`engine::topology::coordinator::`, the rest of `engine::topology::`, and the
rest of `engine::`. On that day it became nine filters and ten shards,
divided by measured time ([`TEST_WINDOWS_HOSTED_JOB`]):

1. `engine::topology::coordinator::tests::a_p`;
2. `engine::topology::coordinator::tests::a_r`;
3. the rest of `engine::topology::coordinator::tests::a`;
4. the rest of `engine::topology::coordinator::`;
5. `engine::topology::recover::`;
6. the rest of `engine::topology::`;
7. the rest of `engine::`;
8. `runner::`;
9. `workspace_manager::`;
10. everything else.

Run 44667503206 measured those ten, and the re-split of 2026-11-20 is made
from it ([`TEST_WINDOWS_HOSTED_JOB`]). It keeps the first, second, fourth
and fifth filters, and `runner::` and `workspace_manager::`, which are now
the seventh and eighth. The rest changed:

- the third filter is `...tests::a_` where it was `...tests::a`, so the
  coordinator's tests that begin with `a` and not with `a_` run in the
  fourth shard;
- the filters `engine::topology::` and `engine::` are gone. The seventh
  shard, the rest of `engine::`, was the slowest in that run, at 878 s in
  the harness and 18.6 minutes a job;
- two filters are new, the sixth and the ninth: `engine::tests::a` and
  `engine::tests::` divide the engine's own `tests` module by the opening
  of a test's name, as the first three divide the coordinator's;
- every other `engine::` test, the rest of `engine::topology::` among
  them, runs in the last shard.

The order is part of the rule. libtest's filter is a substring of a test's
whole path, not a prefix and not a module. Four of the filters contain a
later one -- the first two contain the third, each of the first three
contains the fourth, and the sixth, `engine::tests::a`, contains the ninth,
`engine::tests::` -- and the narrower comes first: placed after the wider
one it would find every test it names already skipped, and its shard would
select nothing. The ninth filter comes after `runner::` and
`workspace_manager::`, which it neither contains nor is contained by, and
that order decides only where a name holding two of the three would run:
in the earliest one's shard, which is still exactly one shard. The same
holds of a name that contains a filter away from its start. On the macOS
and Linux tables a module called `engine` anywhere in the tree puts its
tests in the `engine::` shard; this table has had no `engine::` filter since
2026-11-20, so here such a name runs where its other filters put it, or in
the last shard.
`the_shard_tables_run_every_test_in_exactly_one_shard` holds the ten derived
argument strings to literals, places fourteen names of those kinds in the
shard each belongs to, and holds every filter to one word that libtest reads
as a filter, none of them twice.

## `pub(super) const WINDOWS_SHARD_FLOORS: [u32; 10] = [24, 13, 47, 36, 113, 93, 200, 152, 57, 965];`

The count each Windows shard must report before it may succeed, in shard
order: [`WINDOWS_TEST_FLOOR`] divided among the ten. Each is its leg's
`floor:` in the job's matrix, an integer the oracle holds to this list, and
the number line 10 of [`WINDOWS_SHARD_WITNESS`] tests the leg's count against.

They sum to that floor, 1700, the floor the whole suite was held to as one
job, and `the_shard_tables_run_every_test_in_exactly_one_shard` holds the
sum, one entry for each shard, and every entry above zero. The ten legs
together must not accept fewer passing tests than the one job did; and a
shard whose floor is zero passes having executed nothing, which is the
arrival the count exists to refuse ([`WINDOWS_SHARD_WITNESS`]). That test
holds this list; the oracle holds each leg's `floor:` in `ci.yml` to it.
Measured, `MUT-TEST-WINDOWS-HOSTED-SHARD-FLOOR-LOWERED` and
`MUT-TEST-WINDOWS-HOSTED-SHARD-FLOOR-ZEROED`.

Each floor is about 0.48 of its shard's count of passing tests as the
re-split of 2026-11-20 was estimated: 50, 27, 99, 76, 235, 195, 417, 318,
120 and 2,011. Those are 3,548 together, of which 1700, the floor the whole
suite was held to as one job, is about 0.48 in the same way. The first,
second and fifth shards are shards the
split of 2026-11-19 had, and the eighth, the workspace manager's, was its
ninth; each keeps its count. The fifth's floor is 113 where it was 112,
because the ratio is 1700 of 3,548 where it was 1700 of 3,562. The last
shard's count was the largest before and is larger now, and its floor with
it, because it runs the rest of `engine::topology::` and the rest of
`engine::` as well.

The ten floors of 2026-11-19 were 24, 13, 55, 28, 112, 231, 155, 196, 152
and 734, each about 0.48 of a count likewise: 50, 27, 116, 59, 235, 484,
324, 411, 318 and 1,538, which are 3,562 together. The five floors before
them, 95, 30, 340, 150 and 1085, were each about half of a count. Where
those ten divided the five, the estimate agreed with what the five legs
reported in run 44614925593: the first three counts are that run's 193, the
fourth its 59, and the fifth and sixth its 719.

Run 44736631711 was the first to count the ten of the re-split: 50, 27, 99,
76, 235, 199, 416, 325, 117 and 2,014, which are 3,558 together. Each leg
prints its count beside its floor (line 11 of [`WINDOWS_SHARD_WITNESS`]), so
every run counts them again.

## `pub(super) const MACOS_SHARD_FILTERS: [&str; 4] = [`

The same kind of list for the macOS suite, under the same rule: four
filters, five shards -- `engine::topology::coordinator::tests::a_`, which is
everything in the coordinator's `tests` module whose name begins with `a_`,
the rest of `engine::topology::`, the rest of `engine::`, `runner::`, and
everything else. The `test` job's five macOS legs are derived from it,
through [`TEST_LEGS`]; until the macOS suite was divided nothing in `ci.yml`
read it, and the suite was one leg of the `test` matrix.
`the_shard_tables_run_every_test_in_exactly_one_shard` holds its five
derived argument strings to literals and places the same fourteen names, and
holds its filters to the same shape.

It was two filters and three shards until 2026-11-19: `engine::topology::`,
the rest of `engine::`, and everything else. In run 44614925593 those three
legs took 22.2, 11.3 and 14.3 minutes, and 16.6, 11.8 and 14.5 in run
44516609416, about 4 minutes of each fixed. On that day it became four
filters, divided by measured time as the Windows list is:
`engine::topology::coordinator::`, the rest of `engine::topology::`, the
rest of `engine::`, `runner::`, and everything else, with the slowest of the
five legs predicted at about 14 to 15 minutes.

Run 44667503206 measured those five for the first time: 15.8, 4.7, 8.2, 8.1
and 8.1 minutes a leg, their libtest harnesses 721, 126, 310, 255 and 312 s,
about 3.9 minutes of each leg fixed. The first leg, the coordinator's, took
nearly twice the next slowest. The re-split of 2026-11-20 is made from that
run, as the Windows one is ([`TEST_WINDOWS_HOSTED_JOB`]), and changes the
first filter alone: `engine::topology::coordinator::tests::a_` where it was
`engine::topology::coordinator::`, so the rest of the coordinator's tests
run in the second shard, with the rest of `engine::topology::`. The slowest
of the five legs was predicted at about 11.8 minutes
([`TEST_TIMEOUT_MINUTES`]). Run 44736631711 measured the re-split for the
first time: its slowest leg, the second, took 16.0 minutes.

The table has no floors beside it, where the Windows one has
[`WINDOWS_SHARD_FLOORS`]. The `test` job's legs held no count of passing
tests before the shards and gained none with them. A filter here that
matches no test only moves tests to a later leg; none stops running.

## `pub(super) const LINUX_SHARD_FILTERS: [&str; 1] = ["engine::"];`

The same kind of list for the Linux suite, under the same rule: one filter,
two shards -- `engine::`, and everything else. The `test` job's two Linux
legs are derived from it, through [`TEST_LEGS`].
`the_shard_tables_run_every_test_in_exactly_one_shard` holds its two derived
argument strings to literals and places the same fourteen names, and holds
its filter to the same shape.

Until 2026-11-19 `ubuntu-latest` had no filter and ran the suite whole, as
one leg: 11.4 minutes in run 44614925593 and 13.4 in run 44516609416, about
1.6 of them fixed. Divided by measured time, as the other two tables are,
the slower of its two legs was predicted at about 8 to 9 minutes
([`TEST_TIMEOUT_MINUTES`]). Run 44667503206 measured the two at 7.8 and 8.2
minutes, and the re-split of 2026-11-20 that moved the other two tables left
this one as it was.

No floors here either, for [`MACOS_SHARD_FILTERS`]' reason. A Linux leg
handed the other's arguments is a different matter: the tests that run on
Linux alone then run nowhere, and the matrix check refuses it. Measured,
`MUT-TEST-UBUNTU-LEG-FILTERED`.

## `pub(super) const TEST_JOB_NAME: &str = "test (${{ matrix.leg }})";`

The `test` job's `name:`, an expression, held as the workflow writes it.
GitHub lists each leg under the name with its `leg:` in place:
`test (ubuntu-latest, 1 of 2)` and `test (ubuntu-latest, 2 of 2)`, and
`test (macos-latest, 1 of 5)` to `test (macos-latest, 5 of 5)`. Until the
Linux suite was divided its one leg was `test (ubuntu-latest)`, the name it
had while the name read `matrix.os`; under that expression the legs of one
runner would share one name, and a red one would not say which shard's tests
failed.

No check reads these names. Branch protection requires the aggregate's
context alone ([`REQUIRED_CONTEXT`]), and no script of the workflow reads
another run's jobs now that the picker is gone. Code `test-job-name`.
Measured, `MUT-TEST-LEGS-NAMED-BY-RUNNER`.

## `pub(super) const TEST_TIMEOUT_MINUTES: i64 = 30;`

The `test` job's limit: 30 minutes for every leg, on every lane, and an
integer in `ci.yml`.

While the macOS suite ran whole the limit was an expression, 45 minutes on
the macOS leg and 30 on ubuntu. In two merge-queue entries of one head on
2026-10-29, runs 40834103792 and 40838006947, `test (macos-latest)` exceeded
the 30 minutes it then had and was cancelled; the second job log contains
every closing test summary, each ok, and printed summaries are not a
successful exit. The 45 minutes named no cause for either cancellation and
repaired no test. Under them `test (macos-latest)` was the slowest job of
every run that did not wait for the guest: 32m 23s of run 43705001427's
32m 32s on 2026-11-15, the harness 1,777 s for 3,888 tests on three
processors, 72% of that limit.

The three shards [`MACOS_SHARD_FILTERS`] first made ran on macOS in draft
pull request #339 on 2026-10-18, run 39146387106: 10m 13s, 12m 10s and
12m 17s a leg. The whole leg has taken 27 to 37 minutes since. No shard was
timed at the suite the shards landed on; the CI run of the pull request that
landed them is that measurement.

In run 44614925593 on 2026-11-19 those three took 22.2, 11.3 and 14.3
minutes and the one Linux leg 11.4. The two suites are now divided by
measured time, macOS into five legs and Linux into two
([`MACOS_SHARD_FILTERS`], [`LINUX_SHARD_FILTERS`]). The split of that day
had its slowest leg predicted at about 14 to 15 minutes on macOS and about
8 to 9 on Linux, and run 44667503206 measured it for the first time: 15.8
minutes for the slowest macOS leg and 8.2 for the slower Linux one. The
macOS table was divided again on 2026-11-20, from that run, with its slowest
leg predicted at about 11.8 minutes and measured at 16.0 in run 44736631711.
30 minutes is about two and a half times the slowest shard expected and a
little under twice the one measured, and the limit the Linux leg already
had. Code `test-job-timeout`.
Measured, `MUT-TEST-TIMEOUT-RAISED-FOR-MACOS` and `MUT-TEST-TIMEOUT-RAISED`.

## `pub(super) const TEST_LEGS: [(&str, &[&str]); 2] = [`

The `test` job's legs, as data: each runner that hosts its platform's tests
there, with the ordered filters that divide its suite. `ubuntu-latest` has
[`LINUX_SHARD_FILTERS`] and runs its suite as two legs. `macos-latest` has
[`MACOS_SHARD_FILTERS`] and runs it as five.

The matrix is derived from this table rather than written beside it.
`test_legs` in `super::workflow` makes one leg for each string
`shard_selections` makes of a runner's filters, seven today, and
`test_leg_matrix_complaints` requires the `include:` list of `ci.yml` to be
those legs, entry for entry and in order: each entry's `os:` the runner, its
`leg:` the label [`TEST_JOB_NAME`] reads, and its `tests:` the string
[`TEST_SHARD_COMMAND`] hands to libtest.

A filter list for each runner, not one for the job. What divides a suite is
how long it takes on that runner: the Linux suite, never the slow one
([`TEST_TIMEOUT_MINUTES`]), ran whole and with no filter until 2026-11-19,
and is two legs where the macOS suite is five. Giving `ubuntu-latest` its
filter was an edit to this table and to `ci.yml`, and to the ledger rows
that quote the Linux entries.

The runners here must be the [`CI_TARGETS`] runners that host tests, in
order and once each; the same function refuses any other list. Without that
the table and the workflow could lose a platform together with the equality
between them still holding. Windows is absent for the reason it is absent
from the matrix: its suite runs in [`TEST_WINDOWS_HOSTED_JOB`]. Measured rows
and the two measurements made by hand are under `test_leg_matrix_complaints`
in the workflow notes.

## `pub(super) const MSRV_JOB: &str = "msrv";`

The job that holds this crate to the floor it publishes, and the command it
must run -- character for character, for `--locked`'s sake.

`--locked` is the load-bearing flag and the reason this is a pin rather than
a prefix. Without it Cargo may resolve `Cargo.lock` forward, and this
manifest carries two exact-version pins placed for precisely that hazard:
`globset =0.4.19`, because 0.4.20 raised its MSRV to 1.88, and
`yaml-rust2 =0.12.0`, because 0.12.0 declares 1.85.0 -- this crate's floor
exactly, so its next minor is free to leave the contract. An unlocked MSRV
leg compiles a dependency set no release ships and reports green over a floor
it never tested. `CODING_STANDARDS.md` §2 and `CONTRIBUTING.md` both publish
the command with the flag.

## `pub(super) const RUSTFLAGS_KEY: &str = "RUSTFLAGS";`

The workflow-scope compiler flags, and the two names that decide what any
compilation in this workflow actually sees.

`CODING_STANDARDS.md` §11 makes this load-bearing rather than a convenience.
`unfulfilled_lint_expectations` is warn-by-default, so an `#[expect(...)]` on
a rustc lint "retires a suppression only where warnings are promoted to
errors. `ci.yml` sets `RUSTFLAGS: -D warnings` at workflow scope, so today
that is every leg -- which means narrowing it to a single job would silently
take the self-retirement guarantee with it." The word in that sentence this
contract answers is *silently*: nothing read the setting until now.

The pin is an equality because a `contains` reading is exactly what fails
here. `-D warnings -A clippy::disallowed_methods` contains `-D warnings` and
switches off the effect denylist this whole file exists to enforce.

`CARGO_ENCODED_RUSTFLAGS` is modelled because *effective* is the claim, not
*declared*. Cargo reads it in preference to `RUSTFLAGS` and ignores
`RUSTFLAGS` entirely when it is set, so binding it anywhere in this workflow
is the same defect as rewriting the value -- with the pinned line left in
place to read past.

## `pub(super) const AGGREGATE_JOB: &str = "merge-gate";`

The job that aggregates the gates, and the branch-protection context it
publishes. `MAINTAINING.md` points the external rule at this one name, so a
rename leaves branch protection guarding a context nothing produces.

## `pub(super) const AGGREGATE_SCRIPT: &str = r#"failed=0`

The aggregate's script, pinned.

A pin rather than a family of substring predicates, which is what the
standing row forbids growing more of. The enumerated escape is
`for gate in LINT LINT_WINDOWS MSRV TEST; do : LINT_MACOS` -- a loop that
omits a gate while a `contains` check for the omitted name still passes --
and a pin refuses it outright. The pin is not its own oracle: the loop's gate
list is re-derived from `needs:` below and compared, so this literal being a
faithful copy is itself checked against the job graph.

Every gate must succeed, the hosted shards among them, on every event. One
loop names the six gates and fails for any result of any of them but
`success`, so a skipped `test-windows-hosted` is red like any skipped gate.
Until 2026-11-20 the script set two Windows suite jobs aside, the guest's
and the hosted one, and required exactly one of them to succeed and the
other to be skipped, because the picker sent a run to one. With the guest
retired there is one job, and nothing is set aside.
`the_aggregate_requires_the_hosted_windows_shards_and_every_other_gate`
runs the workflow's script with every gate succeeding, and with each gate in
turn failed, cancelled, skipped or abandoned. Measured, by that test on the
workflow's own script and, as a pin, by
`MUT-AGGREGATE-ACCEPTS-THE-HOSTED-SHARDS-SKIPPED`.

## `pub(super) const GATE_JOB_FIELDS: [&str; 4] = ["name", "runs-on", "steps", "timeout-minutes"];`

The fields a platform Clippy job may declare -- an equality, not a denylist.

`if:` and `continue-on-error:` are absent by construction rather than by
enumeration, which is the difference between this and the whitespace-
normalised `if: false` search it replaces: that search could not refuse
`if: ${{ false }}`, an `env`-driven expression, or any disabling form nobody
had thought of. An unknown field is refused, so the next one costs a review
rather than a finding.

## `pub(super) const STEP_FIELDS: [&str; 5] = ["name", "run", "shell", "uses", "with"];`

The fields a step of a *gate* job may declare. Same reasoning, and `env:` is
absent for a reason of its own: a step-level environment is enough to
retarget the compile the gate performs -- `CARGO_BUILD_TARGET` on the macOS
leg lints a target no `#[cfg(target_os = "macos")]` body belongs to, while
the `run:` scalar still matches character for character. Measured,
`MUT-GATE-STEP-RETARGETED`. The steps that carry an `env:` outside the
aggregate are steps of the two suite-running jobs -- the suite step of the
`test` job and of [`TEST_WINDOWS_HOSTED_JOB`], with the capture and
observation steps after each -- and their maps are pinned whole by
[`TEST_STEP_ENV`] and [`TEST_WINDOWS_STEP_ENV`] rather than admitted by field
name.

## `pub(super) const TEMP_FOLDS_CASE_KEY: &str = "UPSTROKE_TEST_TEMP_FOLDS_CASE";`

The one declaration a test step makes to the suite it runs: that the
temporary directory the suite runs under folds case.
`the_temporary_object_scan_resolves_case_aliases_as_the_filesystem_does`
*requires* its native casefold branch -- Git's lower-case fan-out path
resolving to a stored upper-case directory -- only where this variable is
exactly `1`, and observes the branch everywhere else, so a case-sensitive
volume anywhere the variable is not set takes the negative branch instead of
failing (`PR258-CASEFOLD-EXPECTATION-KEYED-ON-TARGET-OS`: keyed on the target
OS, the requirement failed on a supported case-sensitive volume before the
scan was checked). The declaration is per leg because the filesystems are:
the macOS runner's temporary directory folds case, the ubuntu runner's does
not, and the hosted Windows runner's NTFS does.

## `pub(super) const TEST_STEP_ENV: [(&str, &str); 1] = [(`

The whole `env:` map of the hosted step that runs the suite, as an equality.
The value is an expression over the matrix -- `1` on `macos-latest`, empty on
`ubuntu-latest` -- so one step declares for every leg: it reads `matrix.os`,
which each `include:` entry carries, the five macOS shards' alike
([`TEST_LEGS`]). Pinned whole, the way
[`WORKFLOW_ENV`] and the aggregate's map are: a dropped binding makes the leg
observe the casefold branch instead of requiring it, so a temporary directory
that stopped folding case would pass unreported (undeclared on ext4 the alias
test passes, declared it fails at the declaration,
`34-r6-casefold-declaration.log`); a value other than `1` on
a folding leg does the same, and any other key is the step-level environment
[`STEP_FIELDS`] refuses everywhere else, since one key is enough to retarget
the compile. Measured, `MUT-TEST-CASEFOLD-DECLARATION-DROPPED`,
`MUT-TEST-CASEFOLD-DECLARED-ON-THE-WRONG-LEG` and
`MUT-TEST-STEP-RETARGETED-THROUGH-THE-ADMITTED-ENV`.

The capture step after the suite ([`HOST_ACCEPTANCE_CAPTURE`]) carries the
same map, pinned whole under its own code (`test-capture-step-env`), so the
record is made under the environment the suite ran under. Measured,
`MUT-TEST-CAPTURE-CASEFOLD-DROPPED` and
`MUT-TEST-CAPTURE-RETARGETED-THROUGH-THE-ADMITTED-ENV`.

The container capability observation after it ([`CAPABILITY_OBSERVATION`])
carries the same map whole under `test-capability-step-env`. The contract
runs it in the leg's environment unchanged: no `DOCKER_*` selector and no
`UPSTROKE_REQUIRE_DOCKER`. Measured, `MUT-TEST-CAPABILITY-CASEFOLD-DROPPED`,
`MUT-TEST-CAPABILITY-DOCKER-HOST-INJECTED` and
`MUT-TEST-CAPABILITY-REQUIRE-DOCKER-INJECTED`.

## `pub(super) const TEST_WINDOWS_STEP_ENV: [(&str, &str); 1] = [(TEMP_FOLDS_CASE_KEY, "1")];`

The same declaration on the Windows suite steps, `1` outright: the job's
matrix varies the shard and not the runner, and the hosted runner's NTFS
folds case. The suite step, the capture step and the container capability
observation each carry the map whole, under codes of their own
(`test-windows-hosted-step-env`, `test-windows-hosted-capture-step-env`,
`test-windows-hosted-capability-step-env`), and the observation carries no
other key. Measured, `MUT-TEST-WINDOWS-HOSTED-CASEFOLD-DECLARATION-DROPPED`,
`MUT-TEST-WINDOWS-HOSTED-CAPTURE-CASEFOLD-DROPPED` and
`MUT-TEST-WINDOWS-HOSTED-CAPABILITY-CASEFOLD-DROPPED`. A key put beside the
declaration is measured on the `test` job's steps ([`TEST_STEP_ENV`]); the
row that put one on a Windows observation mutated the guest's job and went
with it on 2026-11-20.

## `pub(super) const TEST_STEP_FIELDS: [&str; 6] = ["env", "name", "run", "shell", "uses", "with"];`

The fields the step that runs the suite, and the capture step after it, may
declare: [`STEP_FIELDS`] plus the `env` whose map [`TEST_STEP_ENV`] and
[`TEST_WINDOWS_STEP_ENV`] pin. Neither may carry `if:` or `continue-on-error:`
(`MUT-TEST-CAPTURE-STEP-DISABLED`, `MUT-TEST-CAPTURE-STEP-ABSOLVED` and, on
the shards' suite step, `MUT-TEST-WINDOWS-HOSTED-SUITE-ON-ONE-LANE`). The
observation step has [`CAPABILITY_STEP_FIELDS`], and every other step of the
two test jobs keeps [`STEP_FIELDS`].

## `pub(super) const AGGREGATE_STEP_FIELDS: [&str; 4] = ["env", "name", "run", "shell"];`

The fields the aggregate's one step may declare: `name`, `run` and `shell`,
plus the `env` whose mapping is pinned key by key -- as the suite steps' maps
in the two test jobs are pinned by [`TEST_STEP_ENV`] and
[`TEST_WINDOWS_STEP_ENV`].

## `pub(super) const AGGREGATE_JOB_FIELDS: [&str; 6] =`

The fields the aggregate declares. `if:` is *required* here and pinned to
`always()` below: without it a failed dependency skips the aggregate, which
leaves the required context missing rather than red.

## `pub(super) const TEST_JOB_FIELDS: [&str; 5] =`

The fields the job that runs these fixtures declares. Each is pinned by
value as well: `name` by [`TEST_JOB_NAME`], `runs-on` as `${{ matrix.os }}`,
`timeout-minutes` by [`TEST_TIMEOUT_MINUTES`], `strategy`, whole, by the
legs [`TEST_LEGS`] derives, and the steps' scripts by [`TEST_SCRIPTS`].

## `pub(super) const TEST_WINDOWS_HOSTED_JOB_FIELDS: [&str; 5] =`

The fields [`TEST_WINDOWS_HOSTED_JOB`] declares: the same five as the `test`
job, each pinned by value as well. `name` is held to
[`TEST_WINDOWS_HOSTED_NAME`], `runs-on` to [`TEST_WINDOWS_PLATFORM`] and
`timeout-minutes` to [`TEST_WINDOWS_HOSTED_TIMEOUT_MINUTES`]. `strategy`
carries the ten legs and is required, not admitted: without it the job is
one leg whose `matrix` expressions are empty. Its contents are pinned whole
by `windows_shard_matrix_complaints` in `super::workflow`.

`needs:` and `if:` are refused like any field the set leaves out. Until
2026-11-20 the set had seven fields and required both, pinned to the
picker's `hosted` answer. The job now waits for no answer, and an `if:` on
it is a Windows suite some event does not run. `continue-on-error:` is
absent by construction, as everywhere in this contract. Measured,
`MUT-TEST-WINDOWS-HOSTED-CONDITIONED`, and
`MUT-JOB-PERMISSIONS-ON-THE-HOSTED-SHARDS`, whose key this set refuses as
well as the permission scan.

## `pub(super) const MSRV_JOB_FIELDS: [&str; 5] =`

The fields the MSRV leg declares. The same shape as the `test` job, and named
separately because the reason one of its absences matters is its own.

`continue-on-error:` is the field this refuses that nothing else would have.
A *disabled* leg (`if: false`) still reports `skipped` to the aggregate,
whose loop demands `success` and fails; an *absolved* leg reports `success`
after its check failed, so the aggregate reads success, `upstroke-ci` settles
green, and the floor `CODING_STANDARDS.md` §2 publishes went unverified on
all three platforms at once.

## `pub(super) const OPTIONAL_DEFAULTS_FIELD: [&str; 1] = ["defaults"];`

The one field a job -- or the workflow itself -- may declare in addition to
its required set.

`defaults:` is optional rather than forbidden because forbidding it would
make the effective-shell resolution below unreachable, and an unreachable
resolution is an untested one. It is modelled instead: declared, its shape is
checked and the shell it resolves to is compared against what this contract
expects for that runner.

## `pub(super) const WORKFLOW_FIELDS: [&str; 6] =`

The workflow's own top-level fields: required, then the same optional one.

## `pub(super) const DEFAULTS_FIELDS: [&str; 1] = ["run"];`

The fields a `defaults:` mapping and its `run:` mapping may declare.

`working-directory:` is **not** among them. GitHub applies a job- or
workflow-level default directory to every `run:` step, so
`working-directory: ci-pass` beside a trivial `ci-pass/Cargo.toml` makes the
pinned test command test that empty crate -- with the runner, the shells,
the checkout and the command all still matching character for character.
No job here needs one, so the field is refused rather than modelled.
Measured, `MUT-DEFAULTS-WORKING-DIRECTORY`.

## `pub(super) const WORKFLOW_ENV: [(&str, &str); 2] = [`

The workflow's `env:` mapping, pinned whole.

An equality rather than a guard per name. [`RUSTFLAGS_KEY`] and
[`ENCODED_RUSTFLAGS_KEY`] were guarded by name because they decide what the
compiler sees; but a *name this contract never thought of* decides whether
the compiled thing runs at all. `CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUNNER`
pointed at a script that exits zero makes `cargo test` build every harness
and execute none, on the Windows target only, which no Unix leg would
notice. Cargo documents that setting as applying to `cargo test`. Pinning
the whole map refuses every such name at once, including the ones nobody
here has thought of yet. Measured, `MUT-WORKFLOW-ENV-TARGET-RUNNER`.

## `pub(super) const WORKFLOW_PERMISSIONS: [(&str, &str); 1] = [("contents", "read")];`

The workflow's `permissions:` mapping, pinned whole.

The trust story rests on this one binding: the job token is read-only, so a
candidate's build scripts and tests cannot push. `permissions: write-all`,
or a `contents: write` here, hands every step of every job a token that can,
and `actions/checkout` leaves that credential configured for the Git
commands a test makes afterwards. Destroying the runner after the job does
not recall a push. Requiring the key to *exist* is not the check; the value
is. Measured, `MUT-WORKFLOW-PERMISSIONS-WIDENED`.

No job declares a `permissions:` of its own. A block on a job replaces this
map for that job, and the oracle refuses one on every job
(`job_permissions_complaints` in the workflow notes). Until 2026-11-20 the
picker's job carried the one exception, `actions: read` beside
`contents: read`, to list other runs' jobs. Measured,
`MUT-JOB-PERMISSIONS-ON-THE-HOSTED-SHARDS`.

## `pub(super) const OVERRIDING_REPO_FILES: [&str; 4] = [`

Repository files that outrank what `ci.yml` says, and must therefore not
exist while this contract reads only `ci.yml`.

A `rust-toolchain.toml` overrides the rustup default the pinned toolchain
action sets, so every bare `cargo` command in every job would run a compiler
the workflow never names -- including the witness that exists to compile
current stable. A `.cargo/config.toml` can bind
`target.<triple>.runner`, which Cargo applies to `cargo test`: every Windows
harness would build and a wrapper would report success without executing
one. Both are invisible to a reader of `ci.yml`. `CLAUDE.md` already states
the project has no toolchain file and selects toolchains at call sites; this
is that convention made enforceable. Adding either file is a deliberate act
that must extend this contract in the same change.

A Rust test holds this, and a Rust test is exactly what a `.cargo/config.toml`
runner switches off. That is a real limit and it is why this is not written
as a defence: it catches the file committed by accident, on a developer's
machine and in CI, and it says in the diff that the convention is deliberate.
The Windows legs are covered instead by [`WINDOWS_SHARD_WITNESS`], which
counts what actually ran.

## `pub(super) const TOOLCHAIN_ACTION: &str = "dtolnay/rust-toolchain@";`

The toolchain every gate and the `test` matrix install, and the action that
installs it.

The action is pinned by commit in [`PINNED_ACTIONS`]; this pins its
**input**, which is the part that decides which compiler runs. A
`lint (windows)` downgraded from `stable` to the shards' pinned `1.97.1`
makes the build witness link the same toolchain the Windows shards already
link, so nothing in CI code-generates the Windows tree on current stable and
a build script emitting a bad link directive only on newer rustc goes green.
The MSRV leg pins its own floor separately, from the manifest, and so does
the Windows suite job its own compiler: its install is held to
[`GOLDEN_IMAGE_TOOLCHAIN`], which [`WINDOWS_SHARD_WITNESS`] asks for before
it runs the suite.
Measured, `MUT-GATE-TOOLCHAIN-DOWNGRADED`.

## `pub(super) const KNOWN_SHELLS: [&str; 6] = ["bash", "cmd", "powershell", "pwsh", "python", "sh"];`

The shell keywords GitHub resolves to an interpreter it defines.

Anything else is a **custom shell**: GitHub builds the command line
`<template> {0}` where `{0}` is the file it wrote the script to. So
`shell: true` runs `true /path/to/script` -- the step succeeds and the script
never executes. On a Clippy gate that is a green job that never linted, and
on the aggregate it is a required check that never read a single result.
Measured, `MUT-GATE-STEP-CUSTOM-SHELL`.

## `pub(super) const AGGREGATE_SHELL: &str = "bash";`

The shell the aggregate's script needs. It is written in bash -- `[[ ]]` and
`${!name}` are bash, not POSIX sh and not PowerShell.

## `pub(super) struct CiTarget {`

A runner CI uses, the target it compiles, and the shell its `run:` steps get.

The tuple is the load-bearing half. What discharges a platform's clause is
not a job *name* but the target whose `#[cfg(...)]` bodies that job compiles,
and [`cfg_regions`] evaluates predicates against the valuations these
invocations actually set rather than collecting the names inside them.

## `pub(super) struct CiTarget` › `pub(super) runner: &'static str,`

The `runs-on:` value.

## `pub(super) struct CiTarget` › `pub(super) triple: &'static str,`

The target tuple that runner's stable toolchain builds by default.

## `pub(super) struct CiTarget` › `pub(super) default_shell: &'static str,`

The shell a `run:` step resolves to on this runner when neither the step,
the job nor the workflow declares one. GitHub's documented default is
`bash` on Linux and macOS and `pwsh` on Windows, and it is pinned rather
than assumed because a resolved shell that is not what this contract
expects is a step that may not run the command at all.

## `pub(super) struct CiTarget` › `pub(super) keys: &'static [(&'static str, &'static str)],`

The `key = "value"` cfg pairs this runner's compilations set. **Every**
key this census models is listed: a key absent from this table is not
set by the invocation, which makes `key = "value"` false rather than
unknown.

## `pub(super) struct CiTarget` › `pub(super) flags: &'static [&'static str],`

The bare cfg flags set by every compilation this runner performs.

## `pub(super) struct CiTarget` › `pub(super) per_invocation_flags: &'static [&'static [&'static str]],`

The flags set by *some* compilation and not others.

`cargo clippy --all-targets` and `cargo test --all-targets` compile the
library twice: once as a library, and once as a test harness with `test`
set. Coverage asks whether **some** invocation compiles the body, so the
valuations are enumerated rather than merged -- merging them would make
`all(test, not(test))` look reachable.
