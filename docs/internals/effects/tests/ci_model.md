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

The command that executes this file's fixtures, character for character.

## `pub(super) const WINDOWS_BUILD_WITNESS: &str = "cargo build --all-targets --all-features";`

The hosted codegen and link witness for the platform whose tests run in
their own job, character for character.

`cargo check` and Clippy type-check and stop before codegen, and the
Windows test leg links with the golden image's toolchain, which the guest
carries and which moves only by re-curation. Without this step nothing on
GitHub's current stable ever
code-generates or links the Windows tree: a Windows-only codegen or link
failure on current stable would pass every hosted leg while the guest, one
stable behind, links and passes. `cargo build --all-targets` rather than
`cargo test --no-run`: the latter builds only test-profile artifacts, so a
`#[cfg(all(windows, not(test)))]` body in a binary is never linked by it,
where `--all-targets` links the library and the binaries **and** their
unit-test harnesses, plus examples and integration tests. It executes
nothing, so the suite's execution stays where the decision record put it.

In the dev profile. `cargo test` builds the test profile, which inherits
from dev and is identical to it until the manifest says otherwise, and that
compile happens in the test leg with the image's toolchain; so what stays
hosted on current stable is the dev-profile compile and link of every
target, and what moves with execution is the test-profile compile. A
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
would test `master` while the labels, the fields, the shell, the input-free
checkout and the pinned test command that follows all still matched.
Measured, `MUT-TEST-WINDOWS-RUN-RETARGETED` and `MUT-TEST-RUN-RETARGETED`.

## `pub(super) const TEST_SCRIPTS: [&str; 4] = [`

Every script the hosted test job may run: the identity script, the suite,
[`HOST_ACCEPTANCE_CAPTURE`] and [`CAPABILITY_OBSERVATION`]. Each is one exact
string; the list grows by one pinned script at a time, and nothing in it is a
pattern.

## `pub(super) const TEST_WINDOWS_SCRIPTS: [&str; 4] = [`

The scripts the Windows test leg runs: the identity script, the suite
with the witness that it executed, [`HOST_ACCEPTANCE_WINDOWS_CAPTURE`] and
[`CAPABILITY_WINDOWS_OBSERVATION`].

Both Windows suite jobs run this one list, the guest's and its hosted twin
[`TEST_WINDOWS_HOSTED_JOB`]'s. The twin's suite step is the guest's, compiler
questions included, because the twin installs the guest's compiler
([`GOLDEN_IMAGE_TOOLCHAIN`]). The two acceptance-record steps carry over
because neither depends on the guest's image: each runs the test it names
and checks the frame it prints, and every container capability class
passes, an absent Docker included.

## `pub(super) const WINDOWS_TEST_FLOOR: u32 = 1700;`

The count the Windows leg's suite must report before the job is allowed to
succeed.

At `cb8ac1f` it reports 1771 there: 1761 from the lib harness, 10 from the
bin one, 0 from the example. The floor sits below that by a margin wide enough that
ordinary churn does not touch it and narrow enough that a suite which
silently stopped running cannot clear it. Dropping this many Windows tests
is a deliberate act, and it edits this number in the same change.

## `pub(super) const WINDOWS_TEST_WITNESS: &str`

The Windows leg's suite step, character for character: a refusal to run on
any compiler but the image's, then the pinned test command, its exit
status, and a count of what libtest reported.

The compiler check comes first because nothing else in the job makes the
claim: no step installs a compiler, so the one that runs is whatever the
guest's image carries and its environment selects. And
the `rustc` on PATH is not what compiles: Cargo selects its compiler
through `RUSTC`, `CARGO_BUILD_RUSTC` or a `build.rustc` in any config it
reads, and the guest's service environment and Cargo home are inputs this
contract cannot see. So the script asks Cargo itself --
`cargo rustc --lib -- --version` runs the compiler Cargo would run and
prints its version, and honours every one of those overrides. The `rustc`
on PATH is a second question rather than a substitute: the suite's own
fixtures spawn bare `rustc` against what Cargo built -- the effect-denial
fixtures here, the log and run-directory tests -- and a PATH rustc of
another version fails them with an incompatible-rlib error, so a lane
whose PATH disagrees with its Cargo is not the same suite either. The
script asks all three, Cargo's compiler, PATH's rustc and PATH's cargo,
and throws unless every one is
[`GOLDEN_IMAGE_TOOLCHAIN`]'s, which asserts what the image carries and the
guest's environment selects; `the_windows_leg_counts_the_tests_it_ran`
holds the number in the script to the constant, so re-curation edits both
or neither.
Measured, `MUT-WINDOWS-WITNESS-COMPILER-CHECK-DROPPED`.

What the three questions bound is the step's own process: a rustup default
that did not move, a `RUSTC` in the service environment, a PATH that names
another install. The test executables get a process of their own, which
Cargo builds: it sets the platform's library-path variable on every one --
on Windows that is `PATH`, its output directories prepended -- and then
applies a Cargo config's `[env]` table, skipping any key it has already
set. That skip is a case-sensitive lookup and Windows' environment is not,
so on the guest `[env] PATH = { value = "...", force = true }` is ignored
and the same table spelled `Path` replaces the fixtures' PATH: all three
questions answer the image's compiler and the fixtures spawn another
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
failed. So no green run of this step has spawned a fixture compiler other
than the one Cargo built with. That bound is the fixtures', not the
questions', and it rests on those four tests running on the Windows leg,
which nothing here pins by name: the floor counts tests, not which ones.

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
actually is. What it bounds is the guest: the machine that executes this platform's
suite, on every lane the picker leaves with it, is provisioned outside the repository, so
its Cargo home and its environment are inputs no reviewer of a diff can
check, and this is the leg saying that it ran what it claims to have run.
Measured, `MUT-WINDOWS-WITNESS-FLOOR-DROPPED` and
`MUT-WINDOWS-WITNESS-COUNT-DROPPED`.

The same step runs on the hosted twin, character for character. There a
step does install the compiler -- the image's version, at step 1 -- and the
three questions are what make that install the claim rather than a hope:
the toolchain action tolerates a failed `rustup default` and verifies only
`rustc +1.97.1`, so it can report success while bare `cargo` is still the
runner's preinstalled stable. The count carries over for the reason it
exists on the guest: the twin's machine is GitHub's, not the build box, but
a Cargo target runner or a `[workspace]` selection reaches it the same way.
Measured on the twin, `MUT-TEST-WINDOWS-HOSTED-COMPILER-CHECK-DROPPED`,
`MUT-TEST-WINDOWS-HOSTED-WITNESS-DROPPED` and
`MUT-TEST-WINDOWS-HOSTED-FLOOR-DROPPED`.

## `pub(super) const HOST_ACCEPTANCE_CAPTURE: &str = r#"set -o pipefail`

The hosted `test` job's capture of the native HOST acceptance record (G7,
root direction `087fea89`), character for character. It runs on both legs in
GitHub's default `bash -e` and must stand immediately after the suite step.
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

The same capture on the guest, under the job's PowerShell, character for
character, and on the hosted twin when the picker sends the run there. It
stands immediately after the suite-and-witness step, so it runs the harness
that step built on the compiler that step asserted.

- `$LASTEXITCODE` carries Cargo's exit through `Tee-Object`, as in
  [`WINDOWS_TEST_WITNESS`].
- `-ccontains` requires the named test's pass line and `-cmatch` requires
  `1 passed; 0 failed; 0 ignored`.
- The framed lines, split on single spaces, must be BEGIN first and END last
  with the same summary, chunk `k` at position `k` with the tag, every chunk
  at its exact length, and the count equal to the lines found.

The last line is the one that throws. A frame check that is computed and not
acted on refuses nothing (`MUT-TEST-WINDOWS-CAPTURE-FRAMING-UNENFORCED`).

No PowerShell runs on the build box, so this script's execution is the
Windows job's evidence -- the guest's, or the twin's on a run sent there --
and not a local one. Measured in the oracle:
`MUT-TEST-WINDOWS-CAPTURE-STEP-DELETED`,
`MUT-TEST-WINDOWS-CAPTURE-IGNORED-NOT-RUN`,
`MUT-TEST-WINDOWS-CAPTURE-EXIT-SWALLOWED`,
`MUT-TEST-WINDOWS-CAPTURE-ZERO-TESTS-ADMITTED`,
`MUT-TEST-WINDOWS-CAPTURE-FRAMING-UNENFORCED` and
`MUT-TEST-WINDOWS-CAPTURE-SEPARATED-FROM-THE-WITNESS`.

## `pub(super) const CAPABILITY_OBSERVATION: &str = r#"set -eo pipefail`

The hosted `test` job's container capability observation (G7, root direction
`ecf15a9c`, contract version 2 under its root audit 2), character for
character. It runs on both legs in GitHub's default `bash -e` and must stand
immediately after [`HOST_ACCEPTANCE_CAPTURE`]. Line by line:

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

The same observation on the guest, under the job's PowerShell, character for
character, and on the hosted twin when the picker sends the run there. It
stands immediately after [`HOST_ACCEPTANCE_WINDOWS_CAPTURE`].

- `$LASTEXITCODE` carries Cargo's exit through `Tee-Object`.
- `-ccontains` requires the named test's pass line, and `-cmatch` requires
  `1 passed; 0 failed; 0 ignored`.
- The framed lines must be BEGIN first and END last with the same summary,
  chunk `k` at position `k` with the tag, every chunk at its exact length, and
  the count equal to the lines found.
- The last line is the one that throws.

No PowerShell runs on the build box, so this script's execution is the
Windows job's evidence -- the guest's, or the twin's on a run sent there --
and not a local one. Measured in the oracle:
`MUT-TEST-WINDOWS-CAPABILITY-STEP-DELETED`,
`MUT-TEST-WINDOWS-CAPABILITY-IGNORED-NOT-RUN`,
`MUT-TEST-WINDOWS-CAPABILITY-FILTER-MOVED`,
`MUT-TEST-WINDOWS-CAPABILITY-EXIT-SWALLOWED`,
`MUT-TEST-WINDOWS-CAPABILITY-ZERO-TESTS-ADMITTED`,
`MUT-TEST-WINDOWS-CAPABILITY-FRAMING-UNENFORCED` and
`MUT-TEST-WINDOWS-CAPABILITY-SEPARATED-FROM-THE-CAPTURE`.

## `pub(super) const CAPABILITY_STEP_FIELDS: [&str; 4] = ["env", "name", "run", "timeout-minutes"];`

The observation step's exact field set, on every job that runs it -- `test`,
the guest's Windows job and its hosted twin -- every field required.

- It has no `if:` and no `continue-on-error:`: the contract places it
  success-only, so a failed suite or capture stops it before it runs.
- It has no `shell:`, `uses:` or `with:`.
- `timeout-minutes` is the one field it carries beyond what the suite's step
  may, and only this step carries it; [`TEST_STEP_FIELDS`] and
  [`STEP_FIELDS`] are unchanged.

Measured, `MUT-TEST-CAPABILITY-STEP-DISABLED`,
`MUT-TEST-CAPABILITY-STEP-RUN-WHEN-NOT-CANCELLED`,
`MUT-TEST-CAPABILITY-STEP-ABSOLVED`, `MUT-TEST-WINDOWS-CAPABILITY-DISABLED`
and `MUT-TEST-WINDOWS-CAPABILITY-ABSOLVED`.

## `pub(super) const CAPABILITY_STEP_TIMEOUT_MINUTES: i64 = 4;`

The step's bound. The questions request at most 135 s (10 + 60 + 20 +
3 x 15), and Cargo's start comes on top. That sum is the requested bounds,
not a proved wall-clock maximum, and the four minutes are an operational
limit, not a guarantee that every future job fits. The tightest leg is the
guest's 20-minute job. Measured, `MUT-TEST-CAPABILITY-TIMEOUT-DROPPED`,
`MUT-TEST-CAPABILITY-TIMEOUT-WIDENED` and
`MUT-TEST-WINDOWS-CAPABILITY-TIMEOUT-DROPPED`.

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

## `pub(super) const TEST_WINDOWS_JOB: &str = "test-windows";`

The job that runs those fixtures for the one platform whose test execution
is not in the `test` matrix, on the curated guest, on every lane -- unless
the run's `pick-windows-home` answers `hosted`, when this job is skipped and
[`TEST_WINDOWS_HOSTED_JOB`] runs the same suite on `windows-latest`.

The Windows suite is spawn- and worktree-heavy, and on `windows-latest` it
was the whole CI wall clock: a median of 12.5 minutes and a tail of 21 on
identical code, because the harness varied 535-1154 s with the host the
runner landed on. On 2026-09-01 it moved to an ephemeral self-hosted guest --
a throwaway overlay of a frozen image, one job per boot, registered with a
single-use just-in-time config -- and a second guest took the merge queue's
builds so a queue entry never waited behind a pull request. Measured again
on 2026-09-15/16: five hosted samples (run 34980597250, at `126b8832`) ran
the harness in 1122-1499 s, 21m 30s to 28m 03s a job, and the guest, on
three green runs of pull request #299, in 346 s, 6m 47s to 6m 55s a job --
three to four times faster. From 2026-09-16 the queue lane ran on
`windows-latest` instead, so that the second guest could be retired from a
box where two guests and the builds contend for one set of cores, under a
45-minute timeout and then a 55-minute one. In October 2026 three queue
entries of one head reached the 45 minutes and two entries of the next head
the 55, each cancelled before its suite reported a complete result, while
the guest ran the full suite of the tree the second of those two entries
tested, as a pull request, inside its own 20-minute timeout. The queue lane
then returned to the guest -- the same labels and the same guest as every
other build, not the retired second one -- as a scheduling repair that
names no cause for the hosted runs and repairs no test. One guest serves
every lane, so a queue entry waits for it behind whatever build holds it --
unless the operator has set `WINGUEST_BUSY`, or has set a
`WINGUEST_QUEUE_LIMIT` the guest's queue has reached, when the run goes to
[`TEST_WINDOWS_HOSTED_JOB`] instead. Neither is set by default. A run
the guest takes runs its commit on the guest, and says no more than that:
the picker answers for each run on its own, so taking the guest for one run
says nothing about where the earlier runs of the same change ran. With
neither variable set every run of a change is on the guest, and a test whose
outcome depends on the machine is not seen first by the push to master. With
either set, a pull request and its queue entry can both pass on the twin and
the push to master be the change's first run on the guest, where such a test
is then first seen; `WINGUEST_BUSY` cleared between the queue entry and the
push is enough. That is why `MAINTAINING.md` step 3 asks for a Windows-only
failure to be run on the other home before it is believed. The second review
of #375 found this paragraph, and the workflow's comment above
`test-windows`, still naming the guest as the machine "every pull request
runs on".

## `pub(super) const TEST_WINDOWS_LABELS: [&str; 3] = ["self-hosted", "windows", "winguest"];`

The labels the guest's job runs on, on every lane: the job's whole
`runs-on:`, compared as a set. An equality because a looser set is a different machine: `[self-hosted,
windows]` admits any Windows runner the account ever registers, and the
third label is what names the curated image. A scalar there is a hosted
runner, and an expression routes some event elsewhere -- the merge queue's
route to `windows-latest` from 2026-09-16 is one. The set itself is held to
`self-hosted` and to no runner [`CI_TARGETS`] names by
`the_guests_windows_job_runs_on_the_self_hosted_guest_its_labels_name`,
so the equality cannot be moved to a hosted machine with both sides edited
together. Measured, `MUT-TEST-WINDOWS-REHOSTED`,
`MUT-TEST-WINDOWS-LABEL-DROPPED` and `MUT-TEST-WINDOWS-QUEUE-REHOSTED`.

## `pub(super) const GOLDEN_IMAGE_TOOLCHAIN: &str = "1.97.1";`

The compiler the golden image carries, and so the one
[`WINDOWS_TEST_WITNESS`] refuses to run without -- not `stable`, which is
ahead of the image (`lint (windows)` installed 1.98.1 on 2026-09-16 against
the image's 1.97.1). The job installs nothing: the image selects the
compiler on every lane, and re-curation moves it, editing this constant in
the same change. While the merge queue ran on `windows-latest` its lane
installed this version, so that the queue and the pull-request lane ran one
compiler; that install went with the lane, and a toolchain install in this
job is now refused outright, since at this very version it would pass the
witness while the workflow, not the image, chose the compiler.

The hosted twin, [`TEST_WINDOWS_HOSTED_JOB`], has no image to carry a
compiler, so it installs this version, exactly, and runs the guest's
[`WINDOWS_TEST_WITNESS`], whose questions assert it. It first installed
`stable`; the first review of #375 found that the split the hosted queue
lane's pin had removed, and the operator pinned the twin to the guest's
compiler on 2026-11-12. A twin on `stable` runs the suite on a compiler the
guest never ran it on, so a test whose outcome depends on the compiler -- a
version-gated test, a std change to Windows process spawning, a new lint in
test code under `-D warnings` -- passes or fails according to how busy the
guest was. With the pin, what differs between the two homes is the machine,
not the compiler. Re-curation moves the twin's install with this constant.
Measured, `MUT-TEST-WINDOWS-HOSTED-TOOLCHAIN-FLOATS` and
`MUT-TEST-WINDOWS-HOSTED-TOOLCHAIN-DOWNGRADED`.
`lint (windows)` compiles current stable on every event and is required by
the aggregate, so a stable that adds a lint between a pull request's green
and its enqueue ejects the entry through that leg; nothing here shields the
queue from that. Measured, `MUT-TEST-WINDOWS-INSTALL-UNCONDITIONAL`,
`MUT-TEST-WINDOWS-HOSTED-INSTALL-RESTORED` and
`MUT-WINDOWS-WITNESS-COMPILER-CHECK-DROPPED`.

## `pub(super) const TEST_WINDOWS_PLATFORM: &str = "windows-latest";`

The [`CI_TARGETS`] runner whose tests run in [`TEST_WINDOWS_JOB`], or in its
hosted twin [`TEST_WINDOWS_HOSTED_JOB`] on this very runner, rather than in
the `test` matrix.
Its shell and cfg valuations carry over to the guest: the image carries
PowerShell 7, so a `run:` step resolves to `pwsh` there exactly as on
`windows-latest`, and it builds the same MSVC tuple.

## `pub(super) const TEST_WINDOWS_NAME: &str = "test (winguest)";`

The guest's job as the API lists it. The API names a job by its `name:`,
not its id, and that is the name [`PICK_WINDOWS_HOME_SCRIPT`] counts, so the
oracle holds [`TEST_WINDOWS_JOB`]'s `name:` to it: under any other name the
guest's queue reads as empty whatever is waiting. Nothing else pins that
`name:`. Measured, `MUT-PICK-WINDOWS-HOME-COUNTS-A-RENAMED-JOB`.

## `pub(super) const TEST_WINDOWS_TIMEOUT_MINUTES: i64 = 20;`

The guest's timeout and condition. 20 minutes, unchanged by the twin and
pinned now so that it stays the guest's own: a stalled guest holds the
build machine. The job needs `pick-windows-home` and runs only when its
answer is `guest`. Measured, `MUT-TEST-WINDOWS-TIMEOUT-RAISED`,
`MUT-TEST-WINDOWS-RUNS-BESIDE-THE-TWIN` and `MUT-TEST-WINDOWS-NEEDS-DROPPED`.

## `pub(super) const TEST_WINDOWS_HOSTED_JOB: &str = "test-windows-hosted";`

The Windows suite's hosted twin: `test (windows-latest)` on GitHub's
`windows-latest`, needing `pick-windows-home` and running only when its
answer is `hosted`. It installs [`GOLDEN_IMAGE_TOOLCHAIN`] with `clippy` at
step 1 and then runs the guest's [`TEST_WINDOWS_SCRIPTS`], so the two homes
run one suite, on one compiler, against one floor. The two conditions
compare the one answer with its two values, so exactly one of the two jobs
runs on any run whose picker succeeded;
`the_hosted_windows_twin_runs_the_guests_suite_on_the_guests_compiler`
holds the install to the version the suite step asserts, and the conditions
to that shape.

55 minutes: the ceiling the hosted Windows suite last ran under. The job
inherits that suite's record -- 21 to 28 minutes a job in September 2026,
and merge-queue entries cancelled at 45 and then 55 minutes in October with
the cause unknown ([`TEST_WINDOWS_JOB`]). Sharding the suite, as draft pull
request #339 proposed, is not done. Measured,
`MUT-TEST-WINDOWS-HOSTED-RUNS-BESIDE-THE-GUEST`,
`MUT-TEST-WINDOWS-HOSTED-NEEDS-DROPPED`,
`MUT-TEST-WINDOWS-HOSTED-TIMEOUT-RAISED` and
`MUT-TEST-WINDOWS-HOSTED-ON-THE-GUEST`.

## `pub(super) const CACHE_ACTION: &str = "Swatinem/rust-cache@";`

The cache action, by prefix, which the hosted twin may not use. A run
restores caches from its own ref or the default branch. A merge-queue
entry's ref is a fresh `gh-readonly-queue/` branch, and `master` saves a
twin cache only on a push the picker itself sent to the twin, which with
routing opt-in is rare; so nearly every twin run would start cold and then
upload, inside its 55 minutes, a cache nothing later reads, spending the
repository's cache space. That was the hosted merge-queue lane's reason for
carrying none before `d3434ace`. The twin first carried one, "like the other
hosted jobs", which the first review of #375 found did not answer that
reason; the operator removed it on 2026-11-12. A pull request sent to the
twin twice is the one run that would have gained. Measured,
`MUT-TEST-WINDOWS-HOSTED-CACHE-RESTORED`.

## `pub(super) const PICK_WINDOWS_HOME_JOB: &str = "pick-windows-home";`

The job that decides where this run's Windows suite goes, `guest` or
`hosted`, and its name, runner and timeout. The guest is one machine on the
build box, shared with the operator's baselines and Upstroke runs; on a busy
day each makes the other fail, and the owner asked on 2026-11-11 for a second
home used only then.

`ubuntu-latest`, because the image carries `gh` and `jq` and the question
needs nothing else; asked from the guest, it would wait in the queue it
measures. Five minutes, because the question is a few API reads and every job
that needs the answer -- the required aggregate among them -- waits on it.

The `name:` is read as well as shown. The API lists a job by its name, and
[`PICK_WINDOWS_HOME_SCRIPT`] reads every other run's job of this name to tell
a run still choosing its home from one that has chosen. Renamed, with the
script left as pinned, every run reads as having no picker and runs that
start together answer alike again. Measured, `MUT-PICK-WINDOWS-HOME-RENAMED`.

## `pub(super) const PICK_WINDOWS_HOME_JOB_FIELDS: [&str; 6] = [`

The picker's fields, an equality like every other job's. `if:` would skip
the job and `continue-on-error:` would report success over a script that
wrote no answer; either leaves the output empty, which names neither home.
`permissions` is here and on no other job's list.

## `pub(super) const PICK_WINDOWS_HOME_PERMISSIONS: [(&str, &str); 2] =`

Exactly `contents: read` and `actions: read`. Listing other runs' jobs needs
`actions: read`, which the workflow's token ([`WORKFLOW_PERMISSIONS`]) does
not carry, and a job-level block *replaces* the workflow's for that job
rather than adding to it, so the block restates `contents: read`. The
workflow-level map is unchanged, and the oracle refuses a `permissions:`
block on any other job: those jobs run the candidate's build scripts and
tests. Measured, `MUT-PICK-WINDOWS-HOME-PERMISSION-WIDENED`,
`MUT-PICK-WINDOWS-HOME-PERMISSION-ADDED` and
`MUT-JOB-PERMISSIONS-ON-THE-GUEST-JOB`.

## `pub(super) const PICK_WINDOWS_HOME_STEP_ID: &str = "pick";`

The step the job's output reads its answer from.

## `pub(super) const PICK_WINDOWS_HOME_OUTPUTS: [(&str, &str); 1] =`

The job's one output, `home`, bound to the step's. Pinned as a whole map: a
consumer reading `needs.pick-windows-home.outputs.home` reads an empty string
when the output is renamed or bound to another step. Measured,
`MUT-PICK-WINDOWS-HOME-OUTPUT-RENAMED`.

## `pub(super) const PICK_WINDOWS_HOME_STEP_FIELDS: [&str; 5] = ["env", "id", "name", "run", "shell"];`

The picker's one step, all five required: no `uses:`, so no action and no
checkout beside a token that reads other runs. Measured,
`MUT-PICK-WINDOWS-HOME-SECOND-STEP` (the step count refuses a second step).

## `pub(super) const PICK_WINDOWS_HOME_SHELL: &str = "bash";`

The shell the script is written for, declared on the step and checked as
resolved, the way the aggregate's is.

## `pub(super) const PICK_WINDOWS_HOME_STEP_ENV: [(&str, &str); 3] = [`

The step's environment, whole: the job's token for `gh`, and the two
repository variables. They reach the script as environment, never spliced
into its text, and each name reads its own variable. Measured,
`MUT-PICK-WINDOWS-HOME-ENV-CROSSED`.

## `pub(super) const PICK_WINDOWS_HOME_SCRIPT: &str = r#"winguest_queue() {`

The picker's script, character for character. It answers `hosted` when
`WINGUEST_BUSY` is exactly `1`; otherwise, when `WINGUEST_QUEUE_LIMIT` is a
positive whole number, when the guest's queue, as the script reads it in the
other runs of this workflow, has reached that limit; otherwise `guest`.
It answers `guest` whenever the API cannot be read, because a failed question
must not move a run off the machine every lane has run on, and it prints what
it found: the variables, the limit, the two counts, the answer.

Routing on the queue is opt-in. With `WINGUEST_QUEUE_LIMIT` unset, empty or
anything but a positive whole number the script counts nothing, calls no API
and answers `guest`, so with neither variable set every run goes to the guest
as it did before the twin. The first review of this job (#375, head
`873ff8c7`) found the default limit of 2 it then had would be reached by one
build and one queue entry, sending runs -- merge-queue entries among them --
to a hosted suite whose October 2026 queue runs were cancelled at their
timeouts, and the operator made the route opt-in on 2026-11-12.
`the_windows_home_is_picked_by_the_one_job_the_contract_pins` holds the one
count behind the limit check. Measured, `MUT-PICK-WINDOWS-HOME-COUNTS-BY-DEFAULT`.

Two gaps, each found by a review of an earlier attempt, shape the reading:

- **A run listed twice.** Queued and in-progress runs are two queries, and a
  run that starts between them is in both. The run ids from both are
  collected and their repeats removed (`sort -u`) before any run's jobs are
  read, so each distinct run is read once; this run is left out.
- **The first page only.** Every listing is read with `gh api --paginate`,
  and its exit is checked: `gh` has already printed the pages before a failed
  one, so a failure on any page answers `guest` rather than counting part of
  the queue.

**The queue is two counts**, since the second review of #375 (head
`e4e931a9`, `PR375-QUEUE-PICKERS-RACE`). The script first counted the
`test (winguest)` jobs queued or in progress in other runs, and nothing else.
Runs that start together read one listing and none of them can reserve the
guest: a guest job needs its run's picker, so while the pickers run no guest
job is in the queue to be counted. Every picker of a burst read the same
number and gave the same answer: all `guest`, which queues the burst on the
one machine the limit was set to protect, or, where the API lists a job still
waiting on its `needs` as `queued`, all `hosted`, which leaves the guest idle.
The reviewer reproduced both with the script and symmetric canned pages at a
limit of 1. A merge queue that builds several entries at once is such a
burst.

The script now reads each other run's jobs once and adds two counts:

- **Held.** The run's `test (winguest)` jobs that are `in_progress`, or
  `queued` while the run lists no unfinished picker. These are in the
  guest's queue and count whatever the run's id. A run that lists no picker
  at all has nothing to wait for, so its queued guest job is in the queue:
  a run of the workflow as it was before this job, or a guest job listed
  without the picker that released it.
- **Ahead.** The run has a lower run id than this one, holds nothing, and has
  not yet shown where its Windows suite goes. That is any of: its picker is
  listed and not `completed`, whatever its guest job shows; a guest job is
  listed in a status other than `queued`, `in_progress` and `completed`; or
  no guest job is listed and either its picker is, finished, or none of its
  jobs is yet. A `queued` guest job beside an unfinished picker is this kind
  and not the first: whatever status the API gives a job still waiting on
  its `needs`, it has not been released to the guest. A run with jobs listed
  and neither a picker nor a guest job among them is not a contender.

The answer is `hosted` when the two together reach the limit. A later run
that has not yet shown its home is deliberately not counted: it counts this
one. The run id is the same number to every reader, so of two runs that read
each other as undecided exactly one, the later, counts the other; with `h`
jobs held, the `i`-th run of a burst by id counts `h + i - 1`, and the
earliest `limit - h` take the guest while the rest go to the twin. The id,
not a start time: `GITHUB_RUN_ID` is in the job's environment and every
listing carries the others', so the order costs no read and cannot tie. A
re-run keeps its id and so its place.

The rule does not depend on how the API lists a job that waits on its
`needs`, which was not observed: absent, in a status before `queued`, or
`queued`, the run's own picker is listed unfinished beside it, and that is
what the script reads. It does depend on the picker's `name:`, which the
oracle pins and `the_windows_home_is_picked_by_the_one_job_the_contract_pins`
holds this text to. And it depends on a guest job skipped by its condition
being listed as `completed`, which is how the API lists a skipped job: a run
sent to the twin that listed no guest job at all would read as still
choosing for as long as it ran, and every later run would defer to it. That
too was not observed against the live API; the picker's log prints the
second count, so a guest left idle behind a count that never falls shows
there.

No reading here is a reservation, and three things stay open:

- **A contender this run could not see.** Only runs listed `queued` or
  `in_progress` are read. A run that had finished and is re-run a moment
  later, or one still `pending` behind its concurrency group, is not listed;
  if its id is lower and it reads before this run's guest job is listed as
  queued, it does not count this run, and both take the guest. The window is
  the seconds between this script's reads and that listing.
- **An earlier run counted ahead that does not take the guest.** It found the
  guest full a moment before, its picker failed or ran to its five-minute
  timeout, or it was cancelled. A later run that deferred to it went to the
  twin with the guest free: one slower suite, not a wrong result.
- **A run that changes listing between the two queries the other way.** A run
  listed by neither query is not read at all. Asking for `queued` before
  `in_progress` covers a run that starts between the two; it does not cover a
  run whose last running job ends between them while its guest job waits, if
  GitHub then lists that run as `queued` again, which was not observed. This
  is older than the two counts and unchanged by them.

The reading costs two requests and one more for every other run listed, so a
burst of `n` runs spends about `n` squared against the job token's hourly
request limit. Past that limit the API cannot be read and every picker
answers `guest`, the fallback, at the very load the limit was set for. Also
older than the two counts, which add no request.

The pin is what makes the executed answers this text's:
`the_windows_home_picker_answers_from_canned_api_pages` runs the workflow's
script against canned pages, one picker at a time and several against one
snapshot. Measured there, and as pins,
`MUT-PICK-WINDOWS-HOME-COUNTS-BY-DEFAULT`,
`MUT-PICK-WINDOWS-HOME-COUNTS-A-RUN-TWICE`,
`MUT-PICK-WINDOWS-HOME-FIRST-PAGE-ONLY`,
`MUT-PICK-WINDOWS-HOME-LATER-PAGE-FAILURE-IGNORED`,
`MUT-PICK-WINDOWS-HOME-UNREADABLE-ANSWERS-HOSTED`,
`MUT-PICK-WINDOWS-HOME-IGNORES-EARLIER-RUNS-STILL-CHOOSING`,
`MUT-PICK-WINDOWS-HOME-COUNTS-LATER-RUNS-STILL-CHOOSING` and
`MUT-PICK-WINDOWS-HOME-HOLDS-A-JOB-ITS-PICKER-HAS-NOT-RELEASED`.

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

Every gate must succeed except the two Windows suite jobs, of which the
picker sends a run to one: the loop collects their two results, and the
script then requires exactly one `success` and one `skipped`. Both skipped,
both run, or either failed or cancelled is red, and so is a failed picker,
which is a gate like the others and skips both. One loop still names every
gate, so the derived-loop check reaches the pair too.
`the_aggregate_requires_exactly_one_windows_suite_job_and_every_other_gate`
runs the workflow's script over every pair of results and every other gate
failing. Measured there, and as pins,
`MUT-AGGREGATE-ACCEPTS-BOTH-WINDOWS-JOBS-SKIPPED` and
`MUT-AGGREGATE-ACCEPTS-BOTH-WINDOWS-JOBS-RUN`.

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
aggregate and the picker ([`PICK_WINDOWS_HOME_STEP_ENV`]) are the steps of
the three suite-running jobs -- the hosted test step and the suite steps of
the guest's Windows job and its hosted twin, with the capture and
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
not, and the guest's NTFS does, as does the hosted Windows runner's the twin
runs on.

## `pub(super) const TEST_STEP_ENV: [(&str, &str); 1] = [(`

The whole `env:` map of the hosted step that runs the suite, as an equality.
The value is an expression over the matrix -- `1` on `macos-latest`, empty on
`ubuntu-latest` -- so one step declares for both legs. Pinned whole, the way
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

The same declaration on the Windows suite steps, `1` outright: each Windows
suite job is one job with no matrix, and both the guest's NTFS and the
hosted runner's fold case. The twin's three steps carry the same map under
codes of their own (`test-windows-hosted-step-env`,
`test-windows-hosted-capture-step-env`,
`test-windows-hosted-capability-step-env`); measured there,
`MUT-TEST-WINDOWS-HOSTED-CASEFOLD-DECLARATION-DROPPED`. On the guest,
measured, `MUT-TEST-WINDOWS-CASEFOLD-DECLARATION-DROPPED`. The guest's capture
step carries it too (`test-windows-capture-step-env`), measured by
`MUT-TEST-WINDOWS-CAPTURE-CASEFOLD-DROPPED`. So does the guest's container
capability observation (`test-windows-capability-step-env`), which carries no
other key. Measured, `MUT-TEST-WINDOWS-CAPABILITY-CASEFOLD-DROPPED` and
`MUT-TEST-WINDOWS-CAPABILITY-DOCKER-CONTEXT-INJECTED`.

## `pub(super) const TEST_STEP_FIELDS: [&str; 6] = ["env", "name", "run", "shell", "uses", "with"];`

The fields the step that runs the suite, and the capture step after it, may
declare: [`STEP_FIELDS`] plus the `env` whose map [`TEST_STEP_ENV`] and
[`TEST_WINDOWS_STEP_ENV`] pin. Neither may carry `if:` or `continue-on-error:`
(`MUT-TEST-CAPTURE-STEP-DISABLED`, `MUT-TEST-CAPTURE-STEP-ABSOLVED`,
`MUT-TEST-WINDOWS-CAPTURE-DISABLED`). Every other step of the three test
jobs keeps [`STEP_FIELDS`].

## `pub(super) const AGGREGATE_STEP_FIELDS: [&str; 4] = ["env", "name", "run", "shell"];`

The fields the aggregate's one step may declare: `name`, `run` and `shell`,
plus the `env` whose mapping is pinned key by key -- as the two suite-running
steps' maps are pinned by [`TEST_STEP_ENV`] and [`TEST_WINDOWS_STEP_ENV`].

## `pub(super) const AGGREGATE_JOB_FIELDS: [&str; 6] =`

The fields the aggregate declares. `if:` is *required* here and pinned to
`always()` below: without it a failed dependency skips the aggregate, which
leaves the required context missing rather than red.

## `pub(super) const TEST_JOB_FIELDS: [&str; 5] =`

The fields the job that runs these fixtures declares.

## `pub(super) const TEST_WINDOWS_JOB_FIELDS: [&str; 6] =`

The fields both Windows suite jobs declare, the guest's and its hosted twin:
the `test` job's set without a `strategy:`, since one job on one machine
needs no matrix, plus `needs:` and `if:`, which tie each to the picker's
answer. Both are required, and their values are pinned exactly
([`TEST_WINDOWS_CONDITION`], [`TEST_WINDOWS_HOSTED_CONDITION`]), so an `if:`
is admitted only as the one condition each job runs under; before the twin
existed the guest's job had no `if:` at all. `continue-on-error:` is absent
by construction, as everywhere in this contract.

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
pinned test command test that empty crate -- with the labels, the shells,
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
commands a test makes afterwards. Destroying the guest afterwards does not
recall a push. Requiring the key to *exist* is not the check; the value is.
Measured, `MUT-WORKFLOW-PERMISSIONS-WIDENED`.

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
The leg whose execution left GitHub's runners is covered instead by
[`WINDOWS_TEST_WITNESS`], which counts what actually ran.

## `pub(super) const TOOLCHAIN_ACTION: &str = "dtolnay/rust-toolchain@";`

The toolchain every gate and the `test` matrix install, and the action that
installs it.

The action is pinned by commit in [`PINNED_ACTIONS`]; this pins its
**input**, which is the part that decides which compiler runs. A
`lint (windows)` downgraded from `stable` to the guest's own `1.97.1` makes
the hosted witness link the same toolchain the test leg already links, so
nothing in CI code-generates the Windows tree on current stable and a build
script emitting a bad link directive only on newer rustc goes green. The
MSRV leg pins its own floor separately, from the manifest, and the Windows
test leg installs nothing: its compiler is the image's, which
[`WINDOWS_TEST_WITNESS`] asks for by [`GOLDEN_IMAGE_TOOLCHAIN`].
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
