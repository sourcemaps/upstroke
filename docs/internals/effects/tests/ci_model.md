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

It is the Linux and the Windows gate, each run natively on the runner whose
tuple it compiles. The macOS gate is [`CLIPPY_GATE_MACOS_CROSS`].

## `pub(super) const CLIPPY_GATE_MACOS_CROSS: &str =`

The macOS Clippy gate: [`CLIPPY_GATE`] with `--target aarch64-apple-darwin`,
run on `ubuntu-latest`. Clippy lints only this package's own code, and `cfg`
follows the target, not the host, so a Linux Clippy for that tuple evaluates
every `#[cfg(target_os = "macos")]` region of the package's target code as a
Mac would. A build script or a proc-macro of this package is a host unit and
would be linted for Linux even under `--target`, so the package has none:
`no_repository_file_adds_a_host_unit_the_macos_cross_checks_cannot_reach`
refuses one, and adding one means restoring a native `macos-latest` Clippy
leg. `lint (macos)` ran on `macos-latest` until GitHub's cap of five macOS
jobs at once for the account met seven per run -- this leg, an `msrv` entry
and the five macOS test legs -- and run 45175187088 showed each macOS job
starting as another ended. The macOS test legs, nine under the category registry,
still run natively, and the
`msrv` entry is native again: [`CiTarget`]'s `msrv_runner` says why.

Dropping the flag leaves a gate that is still green and still exact text for
the Linux one: two jobs then Clippy the Linux tuple and none the macOS
regions. Measured, `MUT-GATE-CROSS-TARGET-DROPPED`.

## `pub(super) const TEST_COMMAND: &str = "cargo test --all-targets --all-features";`

The command that builds and executes this file's fixtures, character for
character: the whole suite, with no selection. No job runs it whole now.
Every leg runs the suite it builds over a selection: the legs of
`test-linux` and `test-macos` as [`TEST_SHARD_COMMAND`], and the Windows
shards' through
[`WINDOWS_SHARD_WITNESS`], whose line 4 starts `cargo` as a process with this
command's arguments and `--no-run` after them, so that Cargo only builds,
and whose line 7 starts each harness Cargo built over the leg's selection.
Until 2026-11-20 the guest's Windows job ran it whole, as one job
([`TEST_WINDOWS_HOSTED_JOB`]).

## `pub(super) const TEST_SHARD_COMMAND: &str =`

The suite step of `test-linux` and `test-macos`, character for character:
[`TEST_COMMAND`], then ` -- ${{ matrix.tests }}`. The expression is text
here, as it is in `ci.yml`; GitHub replaces it with the leg's `tests:` value
from the job's matrix (its row of [`TEST_JOBS`]) before bash reads the line. What follows `--` goes to
libtest in every harness `--all-targets` builds.

- On every leg, Linux or macOS, the value is the positive filters and skips
  derived by `category_legs` from `TEST_CATEGORIES`. The skips preserve
  first-row ownership and the platform partitions; display order is not
  priority. Without the selection all sixteen legs run their platform's whole suite.
- Until the Linux suite was divided its one leg's value was the empty
  string: the line ended at `--`, libtest was handed nothing, and the leg
  ran the whole suite, as it did when the step was [`TEST_COMMAND`] alone.

One step for every leg of a job, so nothing in it varies by leg except
through that expression, and by job except through the case-fold map (each
row's `step_env`, a literal since the split); what the expression
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

## `pub(super) const CATEGORY_CHECK: &str = "bash .github/scripts/test-categories.sh";`

The native inventory check in Linux lint and before every Linux suite.
It lists all targets and features, including ignored registrations, checks
first-match ownership and one generated selection per name, then lists again
with all category filters skipped. The remainder must be empty.

## `pub(super) const CATEGORY_CHECK_WINDOWS: &str = "python .github/scripts/test-categories.py check";`

The same check under the Windows runner's Python, before every suite witness.

## `pub(super) const CATEGORY_CHECK_MACOS: &str = r#"if [[ '${{ matrix.check_inventory }}' == 'true' ]]; then`

The exact conditional script for macOS. It runs the whole native inventory check
before the suite of the generated `check_inventory: true` leg. The generator
assigns that responsibility to the leg containing `unit/cli`, so splitting unit
tests or moving that subcategory between legs preserves one owner. Every other
macOS leg explicitly carries `false`. A Linux inventory cannot establish
macOS-only membership. The workflow oracle counts owners in the actual matrix;
an execution control expands the condition for every leg and observes one call.

## `pub(super) const TEST_SCRIPTS: [&str; 6] = [`

The scripts Linux and macOS test jobs may run: identity setup, the direct and
conditional category checks, the selected suite, host acceptance capture and
container capability observation. These are exact strings, not patterns.
The oracle requires the appropriate check exactly once before the suite.

## `pub(super) const TEST_WINDOWS_HOSTED_SCRIPTS: [&str; 5] = [`

The Windows scripts: identity setup, native category check, suite witness,
host acceptance capture and container capability observation. Each of the
twenty legs runs both record steps after its suite. These remain separate
executions of named ignored registrations, outside the ordinary suite's
passing-test count.

## `pub(super) const WINDOWS_TEST_FLOOR: u32 = 1700;`

The sum of the twenty Windows legs' floors, retained from the whole-suite
execution witness. Each floor sums its selected `TEST_CATEGORIES` partition
contributions; every combined floor is positive.
`the_shard_tables_run_every_test_in_exactly_one_shard` holds the sum to this
constant; the suite script reads its own `matrix.floor`.

The total is a minimum number of passing tests, not an inventory size. The
allocation uses the recorded Windows inventory described under
`TEST_CATEGORIES`. A deliberate change to the total edits the partition
contributions and this constant together.

## `pub(super) const WINDOWS_SHARD_WITNESS: &str`

The suite step of every leg of [`TEST_WINDOWS_HOSTED_JOB`], character for
character: a refusal to run on any compiler but the pinned one; the pinned
test command started as a process with `--no-run`, which builds the test
harnesses and runs none; each harness that build reported, started as a
process over the leg's selection, asked for every test's time and waited on;
their output read from files, the test processes they left running, the
suite's exit status, a count of what libtest reported held to the leg's
floor, and the count printed. The first eleven lines run and clean up the
suite; the counter follows. The three `matrix` expressions
are text here, as they are in `ci.yml`; GitHub replaces each with the leg's
value from the table the job's matrix carries before PowerShell reads the
script.

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
another install. While Cargo ran the harnesses, until the legs reported
their tests' times, each test executable got a process of its own, which
Cargo built: it sets the platform's library-path variable on every one --
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
fixture compiler other than the one Cargo built with. Those effects fixtures
belong to the `self-checks` leg under `TEST_CATEGORIES`. That bound is the
fixtures', not the questions'. Their membership is checked by the category
inventory and selectors; passing-test floors alone do not name tests.

The step now starts the harnesses itself (line 7), and Cargo builds none
of their processes. A harness has the step's own environment, with
`RUSTC_BOOTSTRAP` added, and the step's working directory, the checkout's
root, which is the package's. Cargo prepends none of its output
directories to `PATH`, applies no `[env]` table and sets none of the
`CARGO_*` variables it sets for a test it runs, so a test that read one of
those at run time, rather than through `env!` when it was compiled, would
find it unset on these legs and set on every other. The fixtures find
`rustc` on the `PATH` line 1 asks about, and the `[env]` split above does
not reach them. That is read from the text and from Cargo's documented
behaviour, not run.

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
substitution. A wrapper written to print libtest's start and completed
summary can clear the floor, as can a decoy crate whose `harness = false`
main prints them;
both are a forgery, on the machine or in the diff, and the paragraph below
is where forgeries are bounded. Since the legs report their tests' times
the four runner routes no longer reach them: Cargo only builds, and the
step starts the harnesses itself. The `[workspace]` route and a forgery
still do, and the count stays for them.

What it does not do is bound a hostile candidate, and no step in this file
does. An edit to `ci.yml` deletes this one as easily as the guards it
replaces, and the decision record says so where it says where the boundary
actually is. What it bounds is the machine. The count was written for the
guest, which was provisioned outside the repository, so that its Cargo home
and its environment were inputs no reviewer of a diff could check. It stays
now that the suite runs on GitHub's runners, because a `[workspace]`
selection reaches them the same way, and this is each leg
saying that it ran what it claims to have run. A leg's floor also refuses
what the whole suite's could not see: a shard whose filters matched almost
nothing.
Measured, `MUT-TEST-WINDOWS-HOSTED-FLOOR-DROPPED` and
`MUT-TEST-WINDOWS-HOSTED-WITNESS-DROPPED`.

Between the compiler check and the count is the suite itself, lines 3 to
11: built, then run, and every process the script starts waited on.

3. Three paths in `RUNNER_TEMP`: one for each of the suite's two streams,
   and one for the build's messages.
4. The build. `Start-Process` starts `cargo` with [`TEST_COMMAND`]'s
   arguments and, after them inside the one quoted argument list,
   `--no-run --message-format=json-render-diagnostics`: Cargo compiles and
   links every harness `--all-targets` selects and runs none of them. Its
   standard output, one JSON message per line, goes to the build's file,
   and its standard error, where the compiler's diagnostics are rendered as
   text, to the suite's. It hands back the process (`-PassThru`).
   `$null = $build.Handle` has the process object open its handle while
   Cargo runs, without which `ExitCode` can read empty once the process has
   ended. `WaitForExit()` returns when Cargo's own process ends, and `$code`
   keeps its exit. There is no `-Wait`, which waits for the process's
   descendants as well.
5. The build's failure: its standard error printed, and its code the
   step's exit. Cargo can fail having linked some harnesses, whose messages
   are already in the file, and a script that went on would run those and
   could pass. The errors are read inside a `try` whose `catch` prints a
   failed read in their place, and the exit comes after the `catch`, so a
   read that fails cannot replace Cargo's code. Measured,
   `MUT-TEST-WINDOWS-HOSTED-BUILD-ERRORS-READ-UNGUARDED`.
6. The harnesses: every message in the build's file whose `reason` is
   `compiler-artifact`, compared case-sensitively, whose profile is a
   test's and which names an executable, in the order Cargo reported them.
   Nothing else Cargo built answers to all three. A `harness = false`
   `[[test]]` or `[[bench]]` target built by `cargo test --no-run` reports
   `profile.test` true with an executable, so the collection selects it: a
   probe the operator ran on 2026-11-23 with Rust 1.97.1
   (`evidence/harness-false-probe.json` in the operator's kit).
   An empty list throws.
   With no harness run the lines after it would still fail the leg, on an
   output file that was never written or a count below the floor, but
   without naming the cause; the throw names it: Cargo's messages read
   wrongly, or no harness of this crate built.
7. The run. `RUSTC_BOOTSTRAP` is set in the step's process, and each
   harness is started in turn with `${{ matrix.tests }}` and then
   `-Z unstable-options --report-time` as its one quoted argument list, its
   two streams sent to two `.part` files beside the suite's, and waited on
   as the build is. The selection is the leg's filters, if it has any, and a
   `--skip` for each filter of the legs before it; without it every leg
   runs the whole suite. `$code` keeps each harness's exit as soon as it
   has ended, and only then are the two files appended to the suite's, each
   copy in a `try` of its own: a copy that fails is printed, and its `catch`
   throws again only where the harness passed, so a failed copy cannot end
   the step in place of a failed harness's code. The loop stops at the first harness whose exit is not zero, as Cargo's
   own `test` stops at the first harness that fails. The standard-output
   read is also retained with the executable's name in `$harnessLogs`,
   initialised on line 3, so counting does not lose executable boundaries.
8. One `try` block, which reads and then lists. It prints the two files
   and keeps standard output as `$log` for display; the job log carries
   the suite's output once the loop has ended, not as it runs. It then lists
   every process named `upstroke*` that is still running, with its command
   line, and asks Windows to stop each one; the test harnesses are
   `upstroke-<hash>.exe`. `Stop-Process` is given
   `-ErrorAction SilentlyContinue` and nothing checks that the process
   stopped, so the stop is attempted, not ensured. The listing names a
   process, not the test that started it. A process left running under
   another name is neither listed nor asked to stop.
9. The `catch` for line 8. GitHub runs a `pwsh` step with
   `$ErrorActionPreference = 'stop'`, so a failed read or a failed listing
   ends line 8 where it fails. Where the suite's exit was not zero, the
   `catch` prints that failure and the script goes on to line 10, so the
   failure cannot replace the suite's exit code. Where the suite exited
   zero, it throws the failure again and the step fails: nothing was read
   or listed, so nothing can be counted.
10. The suite's exit, where it is not zero: the code of the harness that
    failed. The line rests on line 7 having kept one, as line 9's test
    does: PowerShell compares an empty value as not equal to zero, and
    `exit` given nothing exits zero, before the count.
11. A throw where line 8 listed anything.

What is said here of `Handle`, `ExitCode`, `-Wait`, `exit`, `break`,
`ConvertFrom-Json`, `Add-Content`, `try` and `catch` was not executed where
it was written: no PowerShell runs on the build box
([`HOST_ACCEPTANCE_WINDOWS_CAPTURE`]). The legs' own runs are the evidence
for it.

Lines 4 to 7 were one line until the legs reported each test's time:
`cargo` started over the leg's selection, building the harnesses and
running each in turn. libtest reports a test's time only under
`--report-time`, which it takes only after `-Z unstable-options`, and on a
stable compiler it takes that only where `RUSTC_BOOTSTRAP` is set in the
harness's own environment. The result line of each test the leg runs then
ends with its time, as `test <name> ... ok <0.123s>`; an ignored test is not
run, and its line carries no time. libtest writes the time apart from
the result, so a child a test starts on the harness's console can write
between them and move the time to the next line; the `mklink` that
`link_dir` in `src/rundir/tests.rs` starts was one, and `link_dir` now
captures its output. The tests were packed into the earlier numbered legs
by those times: on 2026-11-22 by CI run
45045067738's, and again on 2026-11-23 by the mean of each test's times over
CI runs 45045067738, 45161322179 and 45233912312 (`MAINTAINING.md` step 3).
The filters were then rebalanced by the median of each test's times over CI
runs 45259861458, 45265008874, 45269007219, 45273775506 and 45337940696.
The `test result:` lines the count reads are the same with the times as
without them.

`RUSTC_BOOTSTRAP` reaches only the harnesses the build made, and what they
start, because of three crates this one builds: `anyhow`, `thiserror` and
`proc-macro2`. Each one's build script probes for a nightly feature --
`error_generic_member_access` for the first two, `proc_macro_span` for the
third -- by compiling a probe without the variable and, where that fails,
as it does on a stable compiler, and the variable is set, compiling it
again with it; where the second compiles, the build script turns the
feature on. Each also has Cargo run it again whenever the variable
changes. A Cargo that saw the variable would hand it to those build
scripts and to every compiler it ran, and build the three, and every
harness over them, otherwise than the legs of `test-linux` and `test-macos` and a user's
machine do, from a tree nobody changed. That is read from the three build
scripts at the versions `Cargo.lock` held on 2026-11-21; no build here was
made with the variable set.

So the step builds once, with `--no-run`, before the variable is set, and
starts no `cargo` after it. `--no-run` is what lets the step start the
harnesses itself, which is the one way to hand them the variable without
handing it to Cargo: without it `cargo test` runs every harness it built
over the whole suite, with no selection and no times, before the loop runs
the leg's shard. Once, because a second Cargo in the step is a second run
of the suite, and after line 7 a build in bootstrap mode as well. The step
takes the harnesses from the build's own messages (line 6), not from paths
it spells, so they are whatever Cargo built. The variable ends with the
step: a later step runs in a process of its own and does not inherit it,
so the host acceptance capture after this one runs the harness this one
built, and not one rebuilt in bootstrap mode
([`HOST_ACCEPTANCE_WINDOWS_CAPTURE`]).

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
step is written against the inference. Waiting on a process does not depend
on what that process left running, and nothing the script reads is a pipe a
survivor can hold open; whether a leg that leaves a process now ends is the
evidence of the next run in which one does.

What is left running is the step's to report, where it was the runner's to
kill at clean-up. Line 8 runs after a failed suite as after a passed one, so
the processes are listed, and Windows is asked to stop them, either way.
Line 9 keeps a failure in line 8 from replacing the suite's exit code, line
10 gives the failing harness's failure that code, and line 11 fails a suite
that passed and left a process: the job that ran the test names the
process's command line, not the test. If the inference is wrong and a
harness itself does not end, line 8 is never reached, and the job's limit
still ends the leg.
`the_windows_leg_counts_the_tests_it_ran` holds the script to line 4 as
[`TEST_COMMAND`]'s arguments and `--no-run` make it, to line 7 as the leg's
selection and the times make it, and to ten things about the processes it
starts: that it waits on each with its handle taken and `WaitForExit()`, and
never with `-Wait`; pipes nothing into `Tee-Object`; builds with `--no-run`
and exits on the build's failure on the next line; sets `RUSTC_BOOTSTRAP`
only after the build and before the harnesses; collects every harness the
build reported on the line before it starts them, refusing none; hands each
the times after the selection; stops at the first harness that fails and
exits with its code before it counts, having kept the code before it
copies the harness's outputs, each copy in a `try` of its own; lists the processes before that exit
and throws for them after it; opens with lines 1 and 2 whole and holds
`cargo` after them once, on the line that builds, with two processes
started in all; and reads and lists inside the `try` whose `catch` keeps
the suite's code. The ten are `windows_suite_wait_complaints` in the test
notes.
Measured in the oracle, each an edit to the workflow that leaves this text
alone: `MUT-TEST-WINDOWS-HOSTED-SUITE-PIPED-AGAIN`, line 7 piping each
harness into `Tee-Object`, as line 4 once piped Cargo;
`MUT-TEST-WINDOWS-HOSTED-SURVIVOR-ALLOWED`, line 11 removed;
`MUT-TEST-WINDOWS-HOSTED-BUILD-RUNS-THE-SUITE`, line 4 without `--no-run`;
`MUT-TEST-WINDOWS-HOSTED-EMPTY-HARNESS-LIST-ALLOWED`, line 6's throw
removed; `MUT-TEST-WINDOWS-HOSTED-HARNESS-TIMES-DROPPED`, line 7 without
the times; `MUT-TEST-WINDOWS-HOSTED-LOOP-PASSES-A-FAILED-HARNESS` and
`MUT-TEST-WINDOWS-HOSTED-LOOP-DROPS-THE-FAILED-CODE`, line 7's loop going
on past a failed harness, and stopping without keeping its code;
`MUT-TEST-WINDOWS-HOSTED-CODE-KEPT-AFTER-THE-COPIES` and
`MUT-TEST-WINDOWS-HOSTED-OUTPUT-COPY-UNGUARDED`, line 7 keeping the code
only after it copies the two outputs, and copying the standard output
outside a `try`;
`MUT-TEST-WINDOWS-HOSTED-BOOTSTRAP-BEFORE-THE-BUILD` and
`MUT-TEST-WINDOWS-HOSTED-BOOTSTRAP-ON-THE-BUILD-LINE`, the variable set
above the build and on its line; and
`MUT-TEST-WINDOWS-HOSTED-CARGO-AFTER-BOOTSTRAP`, a `cargo` after line 7.
Each is under `ci_test_windows_hosted_job_complaints` in the workflow notes.
Measured against this text itself, each an edit a pin cannot refuse because
the workflow and the constant move together: the suite run a second time,
the `try` and its `catch` removed, an edit for each complaint that came
with the build and the loop, and two for the code kept before the copies
(`the_windows_suite_checks_refuse_a_second_run_and_a_masked_exit` in the
test notes).

The ninth of the ten is the one that refuses a second run of the suite, and
it reads this text's letters. It is drawn unless all of these hold:

1. Line 1 is the compiler-questions line, whole.
2. Line 2 is their refusal, whole, built from [`GOLDEN_IMAGE_TOOLCHAIN`]
   with its dots escaped as line 2 writes them.
3. The line that starts the build holds `cargo`, in any letter case, once,
   and no other line after line 2 holds it at all. The words
   `Cargo compiled the harnesses`, in the floor check's message, and
   `Cargo built no test harness`, in line 6's, are set aside before the
   count.
4. `Start-Process` occurs twice: the build's and the loop's.

A `cargo` after line 7 also runs under `RUSTC_BOOTSTRAP`, and builds in
bootstrap mode whatever it was started for:
`MUT-TEST-WINDOWS-HOSTED-CARGO-AFTER-BOOTSTRAP` starts the doctests there,
in the workflow.

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

After the suite, the count starts on line 12 and has three stages:

1. Require one retained output for every compiled executable. Within each
   output, take the first libtest start and the last complete suite summary.
   Do not pair every nested start: a helper can exit without a summary while
   the parent completes. Require the final summary's passed, failed, ignored
   and measured total to match the first start, with status `ok` and zero
   failures; only its passed count contributes. A complete zero-test harness
   contributes zero. No harnesses, missing output, either boundary missing,
   a result before the first start, a start after the last result, a final
   truncated result or an inconsistent total fails. Each start or result
   marker clears the current result; only a complete summary sets it again.
   A nested start announcing the same total as the first start fails closed.
   A start is libtest's own line, `running N test` or `running N tests`,
   with the ` (shuffle seed: S)` libtest appends when it shuffles, alone on
   its line or directly after a `test NAME ... ` or
   `test NAME - should panic ... ` record that another process's output
   interrupted. Text anywhere else in a line is not a start, so an ignored
   test's reason, which libtest writes after `ignored, `, announces none.
2. Refuse a sum below `${{ matrix.floor }}`. The message names the shard
   and its floor.
3. Print the sum, shard and floor, so a later floor change has a measurement.

An earlier count summed every success summary in the merged output,
including helper processes. Its replacement paired every start with a
result, but the recorded unit library harness starts three helpers and
reports only two helper summaries before its complete outer summary; the
runner library harness starts two helpers and reports only one helper
summary. These are separate executable logs, not successive top-level runs
in a shared log. The boundary count gives unit 14 + 1527 + 0 = 1541 passes
and runner 0 + 0 + 409 = 409 passes. The acceptance reference for Windows engine 4
is 99 top-level passes plus nine helper passes: its floor input is 99,
not 108. The native Windows fixture in `super` exercises this count from
the workflow with those recorded boundaries, nested helpers and a zero
harness, plus incomplete inputs. It starts no selected suite a second time.
The count still follows output display, survivor cleanup and the failing
harness's exit, so a counting refusal cannot replace that exit code.

Mixed stdout carries no process identity. First and last boundaries plus
balanced totals are completion evidence, not proof of the writer of each
line. In particular, an outer five-test start followed by a five-test helper
start and its complete five-test summary is indistinguishable from an outer
completion if the missing summary's provenance is assumed. The existing
incomplete fixture stays rejected: any nested start with the outer total
fails closed, including a completed equal-size helper. Supporting that case
would require separate corroboration of completion. Individual timed test
records (`test <name> ... ok <0.123s>`) are not suite summaries and do not
supply that corroboration. Unobserved or forged helper output is outside
what these mixed lines can establish; the executable's actual exit status
and the operator's archived invocation boundaries remain separate evidence.

Round three of #439 closed two gaps in what the count reads as a start.
It read one only where the line ended at the noun, so a helper libtest
shuffled, whose start ends with its seed, was not seen: an outer three-test
start, a shuffled three-test helper passing all three and a parent that
exited zero before its summary counted the helper's three against a floor
of 3 (`PR439-WINDOWS-SHUFFLED-HELPER-COMPLETION`). And it read one after any
whitespace, so an ignored test whose reason ended in `running 1 test`
announced a nested start of the outer total, and a harness that had
completed failed as ambiguous
(`PR439-WINDOWS-IGNORE-REASON-AS-HARNESS-START`). A reason that holds a line
break and then a start line is still read as a start, since the lines
cannot tell it from one. An extra start after the first can only refuse a
run: it marks the run ambiguous or clears a result a later summary must
set again. The summary matcher is unchanged.

The floor is not in this text. It reaches the script from the matrix, so
the equality over the script holds that the floor check reads `matrix.floor` and
the oracle's check of the matrix holds what each leg's floor is
(`category_legs`' summed Windows contributions); a lowered floor in the workflow leaves this pin
intact and is refused there. The same division holds the selection: this
text says line 7 reads `matrix.tests`, and the matrix check says what each
leg's is (`category_legs`' selectors).
`the_windows_leg_counts_the_tests_it_ran` holds exactly one line to opening with the
test of `matrix.floor`, so the floors the oracle holds the matrix to are
floors a leg tests. The final report is held by the oracle's pin alone.

Measured, for the selection,
`MUT-TEST-WINDOWS-HOSTED-SHARD-SELECTION-DROPPED` and
`MUT-TEST-WINDOWS-HOSTED-FILTER-HARDCODED`, each now an edit to line 7.
Each of the eighteen rows this section names edits the step in the workflow
alone, and each is refused under `test-windows-hosted-command`.

The step was the guest's first. Until 2026-11-20 this record stood under
`WINDOWS_TEST_WITNESS`, the suite step of the guest's one job: ten lines,
the eleven this step had before its legs reported their tests' times
without the last, with no selection on line 4 and with 1700 where line 10
read `matrix.floor`. The hosted job ran that text too, character
for character, until it ran the suite in shards. The constant went with the
guest's job ([`TEST_WINDOWS_HOSTED_JOB`]).

## `pub(super) const HOST_ACCEPTANCE_CAPTURE: &str = r#"set -o pipefail`

The hosted `test-linux` and `test-macos` jobs' capture of the native HOST acceptance record (G7,
root direction `087fea89`), character for character. It runs on every leg in
GitHub's default `bash -e` and must stand immediately after the suite step;
each of the nine macOS legs and the seven Linux legs runs it after its own
share of the suite, so a run prints nine macOS records and seven Linux ones
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
compiler that step asserted. It runs in a process of its own, without the
`RUSTC_BOOTSTRAP` that step set for its harnesses, so its Cargo finds that
build as the step left it and does not rebuild it in bootstrap mode.

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
those edits are measured on the capture of `test-linux` and `test-macos`
([`HOST_ACCEPTANCE_CAPTURE`]).

## `pub(super) const CAPABILITY_OBSERVATION: &str = r#"set -eo pipefail`

The hosted `test-linux` and `test-macos` jobs' container capability observation (G7, root direction
`ecf15a9c`, contract version 2 under its root audit 2), character for
character. It runs on every leg in GitHub's default `bash -e`, the nine
macOS legs and the seven Linux legs each, and must stand immediately after
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
on `test-macos`' observation ([`CAPABILITY_OBSERVATION`]).

## `pub(super) const CAPABILITY_STEP_FIELDS: [&str; 4] = ["env", "name", "run", "timeout-minutes"];`

The observation step's exact field set, on the three jobs that run it,
`test-linux`, `test-macos` and `test-windows-hosted`, every field required.

- It has no `if:` and no `continue-on-error:`: the contract places it
  success-only, so a failed suite or capture stops it before it runs.
- It has no `shell:`, `uses:` or `with:`.
- `timeout-minutes` is the one field it carries beyond what the suite's step
  may, and only this step carries it; [`TEST_STEP_FIELDS`] and
  [`STEP_FIELDS`] are unchanged.

Measured, in `test-macos`, `MUT-TEST-CAPABILITY-STEP-DISABLED`,
`MUT-TEST-CAPABILITY-STEP-RUN-WHEN-NOT-CANCELLED` and
`MUT-TEST-CAPABILITY-STEP-ABSOLVED`, each a field the set leaves out. On the
shards, `MUT-TEST-WINDOWS-HOSTED-CAPABILITY-TIMEOUT-DROPPED`, a required
field gone.

## `pub(super) const CAPABILITY_STEP_TIMEOUT_MINUTES: i64 = 4;`

The step's bound. The questions request at most 135 s (10 + 60 + 20 +
3 x 15), and Cargo's start comes on top. That sum is the requested bounds,
not a proved wall-clock maximum, and the four minutes are an operational
limit, not a guarantee that every future job fits. The tightest leg is a
`test-linux` or `test-macos` leg, at 30 minutes ([`TEST_TIMEOUT_MINUTES`]); until 2026-11-20 it
was the guest's 20-minute job. Measured, `MUT-TEST-CAPABILITY-TIMEOUT-DROPPED`,
`MUT-TEST-CAPABILITY-TIMEOUT-WIDENED` and
`MUT-TEST-WINDOWS-HOSTED-CAPABILITY-TIMEOUT-DROPPED`.

## `pub(super) const FMT_GATE: &str = "cargo fmt --check";`

The formatter gate, character for character.

## `pub(super) const SHELL_GATES: [&str; 7] = [`

The six baseline Bash gates plus the native category inventory check that
`lint` runs from the repository root. The listing check supplements the
unchanged ten-command local baseline.

## `pub(super) const GATE_SCRIPTS: [&str; 11] = [`

Every script a gate job may run, the native and the macOS cross Clippy gate
among them. A union across the three gate jobs rather than a set per job:
`lint (macos)` running the formatter would be redundant, not an escape, and
the one command that must sit on exactly one job, the Windows build witness,
is pinned to its carrier separately; which gate a job is, is decided by its
runner and its Clippy line together.

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

`dtolnay/rust-toolchain` is given `components`, `targets` and `toolchain`.
`targets` installs the standard library of a tuple other than the runner's,
which the macOS cross gate needs; the `msrv` entries give it their own
tuple, so the install and the check read one matrix value. Its values are
held to [`TOOLCHAIN_TARGETS`], as `components`' are to
[`TOOLCHAIN_COMPONENTS`].

## `pub(super) const TOOLCHAIN_COMPONENTS: [&str; 2] = ["clippy", "rustfmt, clippy"];`

The `components` values the toolchain action may be given.

The values, not just the key. The action builds a shell command from this
input and interpolates it into a later Bash line, so a value carrying
`;git fetch origin master && git checkout --detach FETCH_HEAD;` installs
Clippy, checks out another tree, and exits zero — through an allowlisted
action at a pinned commit with an allowlisted input name. Measured,
`MUT-COMPONENTS-INJECTED`.

## `pub(super) const TOOLCHAIN_TARGETS: [&str; 2] = ["aarch64-apple-darwin", MSRV_TOOLCHAIN_TARGETS];`

The `targets` values the toolchain action may be given: the macOS tuple the
cross gate names, and [`MSRV_TOOLCHAIN_TARGETS`], the `msrv` entry's own.
The values are pinned for the reason `components`' are: the action
interpolates this input into the same later Bash line. Which value a job
must give is pinned per job on top of this set: `[gate-toolchain-target]`
for a gate whose command carries `--target`, `[msrv-job-toolchain]` for the
floor.

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
this very runner, rather than in a job of [`TEST_JOBS`]. The job's steps are
read under its shell and cfg valuations: a `run:` step there resolves to
`pwsh`, and it builds the MSVC tuple. The oracle holds the job's `runs-on:`
to this runner and [`TEST_JOBS`]' rows to every other. Measured,
`MUT-TEST-WINDOWS-HOSTED-ON-THE-GUEST` and `MUT-TEST-MATRIX-KEEPS-WINDOWS`.

## `pub(super) const TEST_WINDOWS_HOSTED_JOB: &str = "test-windows-hosted";`

The job that runs the Windows suite, on every event: a pull request, a push
to `master` and a merge-queue entry alike. It is a matrix of twenty category legs on
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
the name with its category, optional partition number and sorted subcategories:
`test (windows-latest, coordinator 1 of 6 (registry_waits, snapshot_registration))`,
for example. The oracle holds the `name:` to the
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

Ten legs from 2026-11-19, divided by measured time, because a run's slowest
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

The re-split of 2026-11-20 was made from that run. Each test's sequential
Linux duration was scaled so that the leg it ran in sums to that leg's
measured harness, and blended half and half with a uniform per-test model;
the filters were then chosen by a search for the slowest leg. The slowest of
the ten was predicted at about 14.4 minutes. Run 44736631711 measured the
re-split for the first time: its slowest leg, the fifth, took 18.1 minutes.

Fourteen legs from 2026-11-22, each but the last named by a list of
filters. They were first packed from the
per-test Windows times of CI run 45045067738 alone, and run 45233912312
measured them. They were re-packed from the mean of each test's measured
times over CI runs 45045067738, 45161322179 and 45233912312, into legs whole
module by whole module, and a module is split by the opening of its tests'
names only where it alone exceeds a leg's budget.

The filters were then rebalanced from the median of each test's time over
CI runs 45259861458, 45265008874, 45269007219, 45273775506 and
45337940696, moving whole filters between legs so that each leg's median job
time would be about ten minutes or less, the target until 2026-11-25. The
rebalance was not timed before it landed: the CI run of the pull request
that landed it was its first measurement.

The preceding layout had twenty-two legs from 2026-11-25, each but the last
named by a list of filters, when every suite was divided again to target
about seven minutes per leg, with the organisation's limits
raised to 500 jobs and 50 macOS jobs. From the five complete runs
45503042270, 45505425187, 45506422808, 45508617577 and 45513139892 a Windows
job's fixed part -- setup, compile, the two record captures and teardown --
is a median of 210 s, and a run spends 4760 s running Windows tests. A leg
takes about the fixed part plus its share; one more leg would save about
9 s, and each of those twenty-two was predicted at 421 to 428 s. The re-pack
was not timed before it landed. These predictions describe that earlier
layout, not measured performance of the category layout.

Single tests set a floor under every leg. A leg takes no less than its
longest test plus the job's fixed minutes, and in run 45045067738 the two
longest single tests were:

- `engine::topology::recover::tests::kill_after_report_before_each_cleanup_step_after_the_effect`, 249.8 s;
- `engine::topology::recover::tests::kill_after_report_before_each_cleanup_step_before_the_effect`, 244.5 s.

Under the re-pack from the three runs, every leg's prediction, about 9
minutes a job, was above that floor. Over the five runs of 2026-11-24 the
first of the two takes about 189 s, so the leg that runs it takes no less
than about 399 s with the fixed part, under the 421 to 428 s each leg is
predicted at.

The aggregate reads the twenty category legs through one result,
`needs.test-windows-hosted.result`, as it reads the `test-linux`,
`test-macos` and `msrv` matrices through one result each, and it accepts `success` alone from it, as
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
2026-11-18, in run 44277078005, and the suite step now waits on each
process it starts where it piped Cargo; what the two runs showed, and what is inferred
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

## `pub(super) struct TestCategory<'a> {`

Rows borrow their labels and filters. The registry supplies static slices; a
positive control can append a filter to its own vector and borrow that snapshot
without replacing the row's existing filters.

One registry row; the field meanings and first-match rule are below.

## `pub(super) const TEST_CATEGORIES: &[TestCategory] = &[`

`TEST_CATEGORIES` replaces the three flat platform filter tables. Each row names
a category, subcategory, substring filters, its macOS leg number, and Windows
partitions as `(leg, passing-test floor contribution, filters)`. An empty partition
filter list inherits the row's filters; it never means an unfiltered test run.
Linux has one leg per category. The first matching row owns a registration,
including an ignored registration. Split topic rows precede their broad residual
row. Windows partitions refine that ownership with substring filters, not a
starts-with predicate. An unassigned or multiply assigned partition fails the check.

## `pub(super) struct CategoryLeg {`

One generated leg: its display `name`, libtest `tests` arguments, Windows
`floor` (zero for Linux and macOS, which enforce no passing-count floor), and
`check_inventory`. The latter is true only for the macOS leg containing `unit/cli`;
only the macOS YAML entries carry it.

## `pub(super) fn category_legs(platform: &str) -> Vec<CategoryLeg> {`

Delegates to `category_legs_for` with `TEST_CATEGORIES`. The latter also accepts
fixture registries for normal filter additions, unit subdivision and rebalancing.

`category_legs` combines the rows into seven Linux, nine macOS and twenty Windows
test legs. Names contain the category, `k of n` only for a split category,
and sorted subcategories in parentheses. Categories are sorted for display,
not ownership. Windows floors sum the selected partition contributions. Earlier categories are
excluded; a broad residual topic excludes earlier pieces owned by another leg.
Repeated categories start at their first row, so unit/model retains precedence
over self-checks/effects. Redundant positive filters are removed without changing
the selected set. These combined selectors are a representation of this inventory:
the native check compares their actual substring membership to first-row ownership
for every name. Later matches are allowed by the first-match rule and reported by `--explain`;
a selection that disagrees with the first owner is an error.

`python3 .github/scripts/test-categories.py generate` replaces only the marked
matrix interiors, preserving all outside bytes and each block's LF or CRLF endings.
A synchronized generation does not write the file. The registry and workflow use
explicit UTF-8. `generate --check` and standalone `check` share the same comparison
and refuse drift; `check` does so before building or accepting any inventory.
The generator reads the literal
Rust table, and an effects test compares its parsed registry and generated legs
with the compiled Rust values, including names, selectors, floors and inventory ownership. The workflow
oracle still checks each actual YAML entry, required aggregate context, suite
command and witness. Its mutation controls still delete, move, widen and narrow
selectors, lower floors, remove legs, change matrix expansion and remove checks.

`bash .github/scripts/test-categories.sh` builds Cargo's native all-target,
all-feature harnesses with `--no-run --message-format=json`, then obtains each
harness's terse list, including ignored tests. A registration is the harness and
name together: identical names in a library and binary each count and receive an
owner. A repeated name within one harness is refused. The check validates exactly
one named selection per registration, then lists every harness with all category
filters skipped. Any remaining registration fails with its harness and the table
location. An empty example harness is permitted; an empty aggregate or failed
listing fails.
On a build box it uses `upstroke-build`; on CI it uses Cargo directly. Windows uses
the same Python implementation. Listing occurs before every Linux and Windows suite;
on macOS the pinned condition runs it before the generated owner's suite. C3's existing equality
between shell gate files and Linux lint invocations is retained: lint also runs the
new shell gate. This adds listing/build overhead to that existing job, not a test
leg. No hosted overhead measurement is claimed.

The standalone `check` builds and lists the native all-target, all-feature
inventory across all harnesses and requires nonzero CLI membership. Its Cargo
build never runs from a test; a fixture supplies artifact records to exercise the
standalone entry point with already built inert harnesses. Its `--explain` option
prints the count for every subcategory and each registration also matched by a
later row, identifying its harness. Separately,
the native-libtest test uses `prove --harness` on the current library executable
to list each generated selector and compare the sets to first-match ownership,
without starting Cargo. A compiled
fixture proves the CLI prefix, ordinary and ignored registrations, and rejection
of an inert ignored registration named `newly_registered_unclassified`. Its bodies
panic if executed; the proof only lists them.

Planning evidence was checked from all three recorded runs, 45732809453,
45733917605 and 45733836381, after the split-family remaps and CLI rename:

| OS | Registered, each run | Ignored, each run | Test legs | Membership delta |
|---|---:|---:|---:|---:|
| Windows | 3,929 | 113 | 20 | 0 |
| macOS | 4,265 | 171 | 9 | 0 |
| Linux | 4,406 | 207 | 7 | 0 |

Every recorded name selects exactly its candidate leg, including all ten
platform-only split-family registrations. The broad workspace prefix in the
kill-path proposal is represented by the incarnations and reclaim matrix-family
prefixes; copying it as a naked libtest filter would have selected ordinary
workspace tests too. Equivalent word prefixes were consolidated within the same
membership. No recorded test changes leg, so the test-weight cost delta is zero.
Windows timing is measured; the Darwin and Linux per-test weights are estimates
calibrated to measured job durations. The planning TSVs are not runtime dependencies.

The total Windows floor remains 1,700. It is apportioned over row/leg partitions
in proportion to the 3,816 recorded non-ignored Windows tests, rounding down and
then assigning remaining units by largest fractional remainder in table order.
Every combined Windows leg has a positive floor. Floors, compiler verification,
per-test reporting, process cleanup and the separate ignored record witnesses
remain enforced. The 1,700 total is a minimum execution witness, not the inventory
size.


## `pub(super) const TEST_JOB_NAME: &str = "test (${{ matrix.leg }})";`

The `name:` expression of `test-linux` and `test-macos`. Each matrix `leg`
contains its runner and the name from `category_legs`, for example
`test (macos-latest, coordinator 1 of 2 (core, pipelines, snapshot_registration, verification_charges))`.
The seven Linux and nine macOS labels say which category and topics a leg runs.
The within-category number appears only for split categories.

No check reads these names. Branch protection requires the aggregate's
context alone ([`REQUIRED_CONTEXT`]), and no script of the workflow reads
another run's jobs now that the picker is gone. Code `test-job-name`.
Measured, `MUT-TEST-LEGS-NAMED-BY-RUNNER`.

## `pub(super) const TEST_TIMEOUT_MINUTES: i64 = 30;`

The limit of `test-linux` and `test-macos`: 30 minutes for every leg, on
every lane, and an integer in `ci.yml`.

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

The first three macOS shards ran on macOS in draft
pull request #339 on 2026-10-18, run 39146387106: 10m 13s, 12m 10s and
12m 17s a leg. The whole leg has taken 27 to 37 minutes since. No shard was
timed at the suite the shards landed on; the CI run of the pull request that
landed them is that measurement.

In run 44614925593 on 2026-11-19 those three took 22.2, 11.3 and 14.3
minutes and the one Linux leg 11.4. The two suites were then divided by
measured time, macOS into five legs and Linux into two. The split of that day
had its slowest leg predicted at about 14 to 15 minutes on macOS and about
8 to 9 on Linux, and run 44667503206 measured it for the first time: 15.8
minutes for the slowest macOS leg and 8.2 for the slower Linux one. The
macOS table was divided again on 2026-11-20, from that run, with its slowest
leg predicted at about 11.8 minutes and measured at 16.0 in run 44736631711.
Its lists were then re-packed, and rebalanced from five runs' medians so
that each leg's median job time would be about ten minutes or less. On
2026-11-25 both suites were divided again targeting about seven minutes
per leg, macOS into twelve legs and Linux into five, the slowest predicted at
426 s on macOS and 268 s on Linux. 30 minutes is about four times the
slowest leg expected and a little under twice the 16.0 measured, and the
limit the Linux leg already had. Code `test-job-timeout`.
Measured, `MUT-TEST-TIMEOUT-RAISED-FOR-MACOS` and `MUT-TEST-TIMEOUT-RAISED`.
The category layout retains the limit. Historical timings above do not measure
its seven Linux and nine macOS legs.

## `pub(super) struct TestJob {`

One row of [`TEST_JOBS`]: a job that runs the suite on one platform, and
what the workflow oracle holds that job to. Until 2026-11-24 one `test` job
ran the Linux and macOS legs together, its runner read from each leg's
`os:` and its case-fold declaration from an expression over that runner;
the owner split it into one job per platform that day. A row carries what
the one job used to work out per leg, so each job's `ci.yml` text is
literal: its runner, its legs and its declaration.

## `pub(super) struct TestJob` › `pub(super) id: &'static str,`

The job's id in `ci.yml`: `test-linux` or `test-macos`. The oracle reads the
job by it, names it in every complaint about that job, and the aggregate
reads its result through it (`TEST_LINUX_RESULT`, `TEST_MACOS_RESULT`).

## `pub(super) struct TestJob` › `pub(super) runner: &'static str,`

The job's `runs-on:`, a literal, and the runner its shells are resolved
under. `test_leg_matrix_complaints` derives each leg's `leg:` label from it.
A job on another runner runs that platform's shards elsewhere: code
`test-job-matrix`, measured by `MUT-TEST-MACOS-ON-LINUX`.

## `pub(super) struct TestJob` › `pub(super) step_env: [(&'static str, &'static str); 1],`

The `env:` map the job's suite, capture and observation steps carry, pinned
whole: [`TEMP_FOLDS_CASE_KEY`] bound to `""` on `test-linux` and to `1` on
`test-macos`, the value each platform's legs received from the one job's
expression before the split. The macOS runner's temporary directory folds
case and the ubuntu runner's does not. The reasons the map is pinned whole
are under [`TEMP_FOLDS_CASE_KEY`].

## `pub(super) const TEST_JOBS: [TestJob; 2] = [`

The Linux and macOS jobs, their runners and their steps' pinned case-fold maps.
The runner chooses a platform in `category_legs`; `TestJob` no longer carries
a separate filter table. `test_legs` in `super::workflow` prefixes that runner
to each generated name and keeps its selector. The oracle requires the actual
`include:` list to equal those entries in order: seven Linux and nine macOS
legs. No entry carries `os:`; the runner is the job's.

The rows' runners must be the [`CI_TARGETS`] runners that host tests, in
order and one job each; `test_jobs_table_complaints` refuses any other
table. Without that the table and the workflow could lose a platform
together with the equality between them still holding. Windows has no row
for the reason it has no leg here: its suite runs in
[`TEST_WINDOWS_HOSTED_JOB`]. Measured rows and the two measurements made by
hand are under `test_leg_matrix_complaints` in the workflow notes.

## `pub(super) const RETIRED_TEST_JOB: &str = "test";`

The id of the one job the Linux and macOS legs had until 2026-11-24. A job
of that id left in the workflow is refused, code `test-job-split`: beside
[`TEST_JOBS`]' jobs it runs legs no row holds to anything, and in their
place it runs every platform's legs under one `runs-on:` the oracle no
longer reads.

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

`--target ${{ matrix.target }}` is the other half of the line. The job checks
the floor for the three targets, one `include:` entry apiece, each on its
target's own platform ([`CiTarget`]'s `msrv_runner`). Without the flag each
entry checks its runner's own tuple, which today is its target's on every
entry; the flag keeps the tuple the check compiles the one the entry names,
so an entry moved to another runner -- as the macOS entry stood on
`ubuntu-latest` until 2026-11-25 -- cannot compile that host's regions in
place of its own. Measured, `MUT-MSRV-COMMAND-UNTARGETED`.

## `pub(super) const MSRV_ENTRY_KEYS: [&str; 2] = ["os", "target"];`

The keys of each `msrv` `include:` entry, with the matrix's one key
([`MSRV_MATRIX_FIELDS`]), the job's `runs-on:` ([`MSRV_RUNS_ON`]) and the
toolchain step's `targets:` ([`MSRV_TOOLCHAIN_TARGETS`]). The entries are the
pairs `(msrv_runner, triple)` over [`CI_TARGETS`], each once; an axis or an
`exclude:` beside `include:` changes which combinations run, so `matrix:`
holds `include:` alone. Measured, `MUT-MSRV-MATRIX-NARROWED`,
`MUT-MSRV-MATRIX-EXCLUDED`, `MUT-MSRV-CROSS-TARGET-DROPPED`,
`MUT-MSRV-MACOS-ON-LINUX` and `MUT-MSRV-TOOLCHAIN-TARGET-DROPPED`.

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
`for gate in LINT LINT_WINDOWS MSRV TEST_LINUX TEST_MACOS TEST_WINDOWS_HOSTED; do : LINT_MACOS`
-- a loop that
omits a gate while a `contains` check for the omitted name still passes --
and a pin refuses it outright. The pin is not its own oracle: the loop's gate
list is re-derived from `needs:` below and compared, so this literal being a
faithful copy is itself checked against the job graph.

Every gate must succeed, the hosted shards among them, on every event. One
loop names the seven gates and fails for any result of any of them but
`success`, so a skipped `test-windows-hosted` is red like any skipped gate.
Until 2026-11-20 the script set two Windows suite jobs aside, the guest's
and the hosted one, and required exactly one of them to succeed and the
other to be skipped, because the picker sent a run to one. With the guest
retired there is one job, and nothing is set aside. Since 2026-11-24 the
Linux and macOS suites are two gates, `TEST_LINUX` and `TEST_MACOS`, where
the one `test` job was `TEST`; `MUT-AGGREGATE-DROPS-TEST-MACOS` measures the
aggregate losing one of them.
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
retarget the compile the gate performs -- `CARGO_BUILD_TARGET` on the Windows
leg lints a target no `#[cfg(windows)]` body belongs to, while the `run:`
scalar still matches character for character. `lint (macos)` names its tuple
with `--target`, which Cargo takes over the variable, so the measured row
stands on `lint (windows)`. Measured,
`MUT-GATE-STEP-RETARGETED`. The steps that carry an `env:` outside the
aggregate are steps of the three suite-running jobs -- the suite step of
each of [`TEST_JOBS`]' jobs and of [`TEST_WINDOWS_HOSTED_JOB`], with the
capture and observation steps after each -- and their maps are pinned whole
by each row's `step_env` and [`TEST_WINDOWS_STEP_ENV`] rather than admitted
by field name.

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
scan was checked). The declaration is per platform because the filesystems
are: the macOS runner's temporary directory folds case, the ubuntu runner's
does not, and the hosted Windows runner's NTFS does.

Each of [`TEST_JOBS`]' rows pins the whole `env:` map of its job's suite
step as an equality: `1` on `test-macos`, the empty string on `test-linux`.
Until 2026-11-24 one job's step declared for every leg through an expression
over `matrix.os`, and its map was pinned as one constant, `TEST_STEP_ENV`;
with a job per platform each job's map is a literal of its own. Pinned
whole, the way
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
declaration is measured on `test-macos`' steps (its row of [`TEST_JOBS`]); the
row that put one on a Windows observation mutated the guest's job and went
with it on 2026-11-20.

## `pub(super) const TEST_STEP_FIELDS: [&str; 6] = ["env", "name", "run", "shell", "uses", "with"];`

The fields the step that runs the suite, and the capture step after it, may
declare: [`STEP_FIELDS`] plus the `env` whose map each row of [`TEST_JOBS`]
and [`TEST_WINDOWS_STEP_ENV`] pin. Neither may carry `if:` or `continue-on-error:`
(`MUT-TEST-CAPTURE-STEP-DISABLED`, `MUT-TEST-CAPTURE-STEP-ABSOLVED` and, on
the shards' suite step, `MUT-TEST-WINDOWS-HOSTED-SUITE-ON-ONE-LANE`). The
observation step has [`CAPABILITY_STEP_FIELDS`], and every other step of the
three test jobs keeps [`STEP_FIELDS`].

## `pub(super) const AGGREGATE_STEP_FIELDS: [&str; 4] = ["env", "name", "run", "shell"];`

The fields the aggregate's one step may declare: `name`, `run` and `shell`,
plus the `env` whose mapping is pinned key by key -- as the suite steps' maps
in the three test jobs are pinned by [`TEST_JOBS`]' rows and
[`TEST_WINDOWS_STEP_ENV`].

## `pub(super) const AGGREGATE_JOB_FIELDS: [&str; 6] =`

The fields the aggregate declares. `if:` is *required* here and pinned to
`always()` below: without it a failed dependency skips the aggregate, which
leaves the required context missing rather than red.

## `pub(super) const TEST_JOB_FIELDS: [&str; 5] =`

The fields each of [`TEST_JOBS`]' jobs declares. Each is pinned by value as
well: `name` by [`TEST_JOB_NAME`], `runs-on` by the row's runner, a literal,
`timeout-minutes` by [`TEST_TIMEOUT_MINUTES`], `strategy`, whole, by the
legs the row derives, and the steps' scripts by [`TEST_SCRIPTS`].

## `pub(super) const TEST_WINDOWS_HOSTED_JOB_FIELDS: [&str; 5] =`

The fields [`TEST_WINDOWS_HOSTED_JOB`] declares: the same five as `test-linux`
and `test-macos`, each pinned by value as well. `name` is held to
[`TEST_WINDOWS_HOSTED_NAME`], `runs-on` to [`TEST_WINDOWS_PLATFORM`] and
`timeout-minutes` to [`TEST_WINDOWS_HOSTED_TIMEOUT_MINUTES`]. `strategy`
carries the twenty category legs and is required, not admitted: without it the job is
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

The fields the MSRV leg declares. The same shape as the Linux and macOS test jobs, and named
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
notice. Cargo documents that setting as applying to `cargo test`. The
Windows legs' suite step no longer runs its harnesses through `cargo test`:
it builds with `--no-run` and starts them itself, which a runner does not
reach. The two capture steps after it still run `cargo test`. Pinning
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
`target.<triple>.runner`, which Cargo applies to `cargo test`: every harness
of a leg that runs `cargo test` would build and a wrapper would report
success without executing one. The Windows legs no longer do: they build
with `--no-run` and start the harnesses themselves, which a runner does not
reach ([`WINDOWS_SHARD_WITNESS`]). Both are invisible to a reader of
`ci.yml`. `CLAUDE.md` already states
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

The toolchain every gate and the Linux and macOS test jobs install, and the action that
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

A target CI compiles, the runner whose test legs run it, the runner and the
command of its Clippy gate, the runner of its MSRV check, and the shell its
`run:` steps get.

The tuple is the load-bearing half. What discharges a platform's clause is
not a job *name* but the target whose `#[cfg(...)]` bodies that job compiles,
and [`cfg_regions`] evaluates predicates against the valuations these
invocations actually set rather than collecting the names inside them.

## `pub(super) struct CiTarget` › `pub(super) runner: &'static str,`

The `runs-on:` value of the test legs that compile and run this target
natively, and the label the cfg census in `cfg.rs` reports a predicate's
compilers by. For macOS it stays `macos-latest`: the nine test legs run there.

## `pub(super) struct CiTarget` › `pub(super) check_runner: &'static str,`

The `runs-on:` value of the job that runs this target's Clippy gate. Linux
and Windows are linted on their own runners; macOS on `ubuntu-latest`, by
tuple. It moved there when GitHub ran at most five macOS jobs at once for
the account and each run asked for seven (run 45175187088), and it stays:
Clippy lints only this package's own code, whose `cfg` follows the target.
The gate's shell is resolved for this runner, not for `runner`. Measured,
`MUT-GATE-RUNNER-DRIFT`. The `msrv` entry's runner is `msrv_runner`, which
for macOS differs.

## `pub(super) struct CiTarget` › `pub(super) msrv_runner: &'static str,`

The `os:` of the `msrv` `include:` entry that checks this target's tuple on
the floor, and the runner each of that job's `run:` steps has its shell
resolved for. It is the target's own platform for all three:
`ubuntu-latest`, `windows-latest` and `macos-latest`.

Natively, because the floor is also the dependencies'. Under `--target`
Cargo builds every build script and procedural macro, and what they depend
on, for the runner rather than for the target, so the macOS entry on
`ubuntu-latest` checked the package's macOS regions at 1.85 and compiled
every dependency host unit for Linux; a locked dependency whose macOS host
code needs a newer Rust passed there and fails on a user's Mac
(`PR412-MACOS-DEPENDENCY-HOST-MSRV-GAP`). On `macos-latest` Cargo builds
them for macOS at 1.85, as that Mac does. The entry returned there on
2026-11-25, once the organisation's macOS job limit, raised from 5 to 50
with its move to GitHub Enterprise Cloud on 2026-11-24, was seen in effect.

It differs from `check_runner` for macOS alone; the Clippy gate has no
dependency host units to lint. Measured, `MUT-MSRV-MACOS-ON-LINUX`.

## `pub(super) struct CiTarget` › `pub(super) clippy_gate: &'static str,`

The exact `run:` line of this target's Clippy gate: [`CLIPPY_GATE`] where the
check runner builds the tuple by default, [`CLIPPY_GATE_MACOS_CROSS`] for
macOS. Where it carries `--target <triple>`, the gate job's toolchain step
must install that tuple (`[gate-toolchain-target]`), and
`every_platform_this_crate_configures_for_has_a_clippy_gate_the_aggregate_requires`
holds the triple to `triple`. Measured, `MUT-GATE-TOOLCHAIN-TARGET-DROPPED`.

## `pub(super) struct CiTarget` › `pub(super) triple: &'static str,`

The target tuple that `runner`'s stable toolchain builds by default, and the
one the target's Clippy gate and `msrv` entry compile.

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

