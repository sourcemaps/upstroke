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
Windows test leg links with the golden image's toolchain on both of its
lanes -- the guest carries it, the hosted lane installs it -- which moves
only by re-curation. Without this step nothing on GitHub's current stable ever
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

## `pub(super) const TEST_SCRIPTS: [&str; 3] = [GIT_IDENTITY_SCRIPT, TEST_COMMAND, TEST_SHARD_SCRIPT];`

Every script a test job may run: the identity script, the whole suite, and
one shard of it through the partition runner.

## `pub(super) const TEST_WINDOWS_SCRIPTS: [&str; 3] = [`

The scripts the Windows test legs run: the identity script, the whole suite
with the witness that it executed on the guest's leg, and one shard through
the partition runner on each hosted leg.

## `pub(super) const TEST_SHARD_SCRIPT: &str =`

The macOS shard step's script, character for character: the partition
runner, `.github/scripts/partition.py run`, for the `macos` group and this
leg's shard, recording into `test-partition`. The runner runs
[`TEST_COMMAND`] five times -- build, list, list the ignored tests, list this
shard's selection, run it -- with only the shard's libtest filters from
[`PARTITION_TABLE`] after `--`, and records each command's argv, exit status
and ordered output with the checkout, the run and the compiler. Cargo still
chooses the targets, their working directory and their environment, so the
shard is the suite as the other legs run it, narrowed by name and nothing
else. The shard is interpolated from the matrix rather than passed through
the environment, because any variable the step adds reaches every test the
suite runs. Measured, `MUT-TEST-SHARD-STEP-DELETED`.

## `pub(super) const WINDOWS_SHARD_SCRIPT: &str`

The hosted Windows shard step's script, character for character: the same
two compiler-check lines [`WINDOWS_TEST_WITNESS`] opens with, then the
partition runner for the `windows` group and this leg's shard. The witness's
count is not repeated: the `test-coverage` job refuses the run unless the
shards together ran every listed test, and the Windows group's floor in the
table is [`WINDOWS_TEST_FLOOR`]. Measured,
`MUT-TEST-WINDOWS-SHARD-STEP-DELETED`, `MUT-TEST-WINDOWS-SHARD-UNFILTERED` and
`MUT-TEST-WINDOWS-SHARD-WITHOUT-COMPILER-CHECK`.

## `pub(super) const WINDOWS_COMPILER_CHECK_LINES: usize = 2;`

How many of [`WINDOWS_TEST_WITNESS`]'s leading lines are its compiler check.
[`WINDOWS_SHARD_SCRIPT`] must open with exactly those lines, so the two
Windows suite steps cannot come to refuse different compilers.

## `pub(super) const WINDOWS_TEST_FLOOR: u32 = 1700;`

The count the Windows leg's suite must report before the job is allowed to
succeed, on either lane.

At `cb8ac1f` it reports 1771 there: 1761 from the lib harness, 10 from the
bin one, 0 from the example. The floor sits below that by a margin wide enough that
ordinary churn does not touch it and narrow enough that a suite which
silently stopped running cannot clear it. Dropping this many Windows tests
is a deliberate act, and it edits this number in the same change.

## `pub(super) const WINDOWS_TEST_WITNESS: &str`

The Windows leg's suite step, character for character: a refusal to run on
any compiler but the image's, then the pinned test command, its exit
status, and a count of what libtest reported. One step for both lanes: the
count is the same claim on either machine, and a step per lane would be two
scripts to pin and a condition on each.

The compiler check comes first because the install step cannot make the
claim on its own. `dtolnay/rust-toolchain` at the pinned commit runs
`rustup default` under `continue-on-error` and then verifies only
`rustc +<toolchain>`, so on the hosted lane the install can report success
while bare `cargo` still resolves to the runner's preinstalled stable. And
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
[`GOLDEN_IMAGE_TOOLCHAIN`]'s: on the guest that asserts what the image
carries and its environment selects, on the hosted lane what the install
selected, and `the_windows_leg_counts_the_tests_it_ran` holds the number
in the script to the constant, so re-curation edits both or neither.
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
suite on the pull-request lane is provisioned outside the repository, so
its Cargo home and its environment are inputs no reviewer of a diff can
check, and this is the leg saying that it ran what it claims to have run.
On the hosted lane it bounds nothing the other hosted legs do not already
trust, and it costs nothing to keep.
Measured, `MUT-WINDOWS-WITNESS-FLOOR-DROPPED` and
`MUT-WINDOWS-WITNESS-COUNT-DROPPED`.

## `pub(super) const FMT_GATE: &str = "cargo fmt --check";`

The formatter gate and the seven shell gates the `lint` job runs from the
repository root, character for character. The seventh, `test-partition.sh`,
is the fixture gate of the partition runner and verifier.

## `pub(super) const GATE_SCRIPTS: [&str; 10] = [`

Every script a gate job may run. A union across the three gate jobs rather
than a set per job: `lint (macos)` running the formatter would be
redundant, not an escape, and the one command that must sit on exactly one
job, the Windows build witness, is pinned to its carrier separately.

## `pub(super) const UPLOAD_ACTION: &str =`

`actions/upload-artifact` at the commit of its `v7.0.1` tag, which the
partitioned legs upload their records with.

## `pub(super) const DOWNLOAD_ACTION: &str =`

`actions/download-artifact` at the commit of its `v8.0.1` tag, which the
`test-coverage` job downloads the records with. From `v8` a digest mismatch
against the server's record of the artifact fails the step by default.

## `pub(super) const PINNED_ACTIONS: [&str; 5] = [`

The actions a step may `uses:`, each pinned to the commit it was reviewed
at. An action off this list is code nobody here reviewed running with a
checkout of its own; a floating tag (`@v4`) is that action at whatever
commit the tag points to tomorrow. Measured, `MUT-STEP-USES-UNPINNED`.

## `pub(super) const ACTION_INPUTS: [(&str, &[&str]); 5] = [`

The inputs each pinned action may be given, by action prefix.

Pinning the commit says which code runs; this says what it is told to do,
and the difference is not academic. `Swatinem/rust-cache` accepts a
`cmd-format` input and wraps the commands it runs in it, so a cache step
with one input can run `git checkout` against another tree before Clippy and
the build witness ever see the candidate -- with the action allowlisted, the
checkout input-free, and every `run:` still exact. An input this contract
does not model is refused for the same reason an unknown field is.
Measured, `MUT-CACHE-CMD-FORMAT`. The two artifact actions are admitted the
inputs their one use needs and no more: an upload's name, path and
missing-file behaviour, and a download's pattern and path. A download given a
`run-id` and a token reads another run's artifacts. Measured,
`MUT-COVERAGE-OTHER-RUN`.

## `pub(super) const UPLOAD_ACTION_PREFIX: &str = "actions/upload-artifact@";`

What marks a step as the upload of a shard's record, pinned or not, so an
unpinned upload is still read as one and refused as unpinned.

## `pub(super) const SHARD_UPLOAD_INPUTS: [(&str, &str); 3] = [`

The upload's inputs, whole. The name carries the shard and the run attempt:
artifact names are unique within a run, so a re-run of a leg uploads beside
the record it supersedes rather than colliding with it, and the verifier can
tell the two apart. The path is the directory the runner wrote, and an upload
that finds nothing fails the leg. Measured,
`MUT-TEST-WINDOWS-UPLOAD-WITHOUT-ATTEMPT`.

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
is not in the `test` matrix, and the two machines it runs on, by lane.

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
three to four times faster. Since 2026-09-16 the queue lane runs on
`windows-latest` after all: nothing waits on the queue interactively, six to
ten merges a day are a tenth to a sixth of the 57 a serial lane at 25
minutes an entry can take in a day, bursts queue rather than fail, and the
second guest can then be retired from a box where two guests and
the builds contend for one set of cores. The pull-request lane keeps the
guest, because a pull request waits on it. What the queue's hosted run does
not prove is behaviour that differs between the two machines: a test whose
outcome depends on the runner passes on `windows-latest` in the queue and
fails on the guest, and the push to master, which runs the guest, is the
first run to see it. The compiler pin and the witness's compiler questions
remove the compiler a lane selects by accident -- a default that did not
move, an environment override, a PATH naming another install -- and a
compiler split between Cargo and the fixtures fails the suite rather than
passing it. None of that closes the gap: the two machines' runtime
differences remain.

Since #339 the hosted queue lane runs the suite as four shards, each a leg of
the job's matrix under the same 55-minute ceiling: two queue entries were
cancelled at 55 minutes with the whole suite on one hosted leg. The machine
follows the leg -- the whole suite, `all`, on the guest and every shard on
`windows-latest` -- so `runs-on:`, the ceiling and the install are keyed on
the shard, and the event decides only which legs the matrix holds.

## `pub(super) const QUEUE_LANE: &str = "github.event_name == 'merge_group'";`

The event test that decides which legs the Windows matrix holds: the
`windows` shards of [`PARTITION_TABLE`] where it holds, the guest's whole
suite where it does not. The expression is derived from the table, so a
shard the table drops or adds is a matrix the contract refuses. Measured,
`MUT-TEST-WINDOWS-SHARD-DROPPED`, `MUT-TEST-WINDOWS-SHARD-ADDED`,
`MUT-TEST-WINDOWS-QUEUE-UNSHARDED` and `MUT-TEST-WINDOWS-PR-LANE-SHARDED`.

## `pub(super) const HOSTED_LANE: &str = "matrix.shard != 'all'";`

The hosted-leg condition, spelled once and pinned in five places: the
`runs-on:` expression selects the hosted runner where it holds, the ceiling
gives it the hosted lane's minutes, and the toolchain install, the shard step
and the upload carry it as their `if:`. They are held to one string because
they have to name the same legs: inverted in one place and not another, a
shard runs on whatever GitHub's image preinstalled, or the guest installs
over its own curated compiler, with every other pin matching. Measured,
`MUT-TEST-WINDOWS-LANES-SWAPPED` and
`MUT-TEST-WINDOWS-INSTALL-ON-THE-WRONG-LANE`.

## `pub(super) const WHOLE_SUITE_LANE: &str = "matrix.shard == 'all'";`

The whole-suite leg's condition, the complement of [`HOSTED_LANE`], on the
step that runs the whole suite in both test jobs. The two suite steps carry
complementary conditions, so every leg runs exactly one of them. Measured,
`MUT-TEST-WINDOWS-DISABLED`, `MUT-TEST-WINDOWS-SUITE-ON-ONE-LANE` and
`MUT-TEST-WHOLE-SUITE-ON-EVERY-LEG`.

## `pub(super) const WHOLE_SUITE: &str = "all";`

The matrix value of a leg that runs the whole suite. The partition table may
not name a shard `all`.

## `pub(super) const TEST_WINDOWS_LABELS: [&str; 3] = ["self-hosted", "windows", "winguest"];`

The labels the pull-request and push lane runs on. An equality inside the
expression because a looser set is a different machine: `[self-hosted,
windows]` admits any Windows runner the account ever registers, and the
third label is what names the curated image. Measured,
`MUT-TEST-WINDOWS-LABEL-DROPPED`.

## `pub(super) const TEST_WINDOWS_NAME: &str = "test (${{ matrix.shard == 'all' && 'winguest' || format('windows-latest, {0}', matrix.shard) }})";`

The job's name, character for character: `test (winguest)` for the guest's
leg, the name the scheduling guards read, and `test (windows-latest, <shard>)`
for each hosted shard, so no two legs publish one name. Measured,
`MUT-TEST-WINDOWS-GUEST-RENAMED` and `MUT-TEST-WINDOWS-SHARD-NAMES-COLLAPSE`.

## `pub(super) const TEST_WINDOWS_RUNS_ON: &str`

The job's `runs-on:`, character for character: a ternary over the hosted-leg
condition that yields the hosted runner's name for a shard and the label set,
through `fromJSON`, for the whole-suite leg. A scalar `windows-latest` sends
the pull-request lane to the hosted runner and its six minutes become
twenty-five; a bare label set sends every leg to the pull-request guest,
where a shard's install -- conditioned on the shard, not the machine -- runs
over the image's compiler. The literal is
held to [`HOSTED_LANE`], [`TEST_WINDOWS_PLATFORM`] and
[`TEST_WINDOWS_LABELS`] by
`the_windows_leg_routes_each_lane_to_the_runner_its_install_step_is_written_for`,
so the three cannot drift apart while the pin still reads back. Measured,
`MUT-TEST-WINDOWS-REHOSTED` and `MUT-TEST-WINDOWS-QUEUE-LANE-REGUESTED`.

## `pub(super) const TEST_WINDOWS_TIMEOUT: &str = "${{ matrix.shard != 'all' && 55 || 20 }}";`

The job's ceilings, unchanged by the partition: the hosted lane's 55 minutes
for every shard and the guest's 20 for its whole suite. Measured,
`MUT-TEST-WINDOWS-CEILINGS-SWAPPED`.

## `pub(super) const TEST_NAME: &str = "test (${{ matrix.shard == 'all' && matrix.os || format('{0}, {1}', matrix.os, matrix.shard) }})";`

The `test` job's name: `test (ubuntu-latest)` for the whole-suite leg and
`test (macos-latest, <shard>)` for each macOS shard. Three legs share the
macOS runner, so a name without the shard is three checks under one name.
Measured, `MUT-TEST-NAME-WITHOUT-SHARD`.

## `pub(super) const TEST_WHOLE_SUITE_RUNNER: &str = "ubuntu-latest";`

The one `test` leg that runs the whole suite.

## `pub(super) const TEST_SHARD_RUNNER: &str = "macos-latest";`

The runner of every `macos` shard. Together with
[`TEST_WHOLE_SUITE_RUNNER`] it must be exactly the platforms that host these
fixtures in the `test` job, so the partition moves no platform in or out.

## `pub(super) const GOLDEN_IMAGE_TOOLCHAIN: &str = "1.97.1";`

The compiler the golden image carries, and so the one the hosted lane
installs and the one [`WINDOWS_TEST_WITNESS`] refuses to run without, on
either lane -- not `stable`. Current stable is ahead of the image
(`lint (windows)` installed 1.98.1 on 2026-09-16 against the image's
1.97.1), so `stable` here would run the suite on a compiler the
pull-request lane never ran it on: a test whose outcome depends on the
compiler -- a version-gated test, a std behaviour change, a new lint in
test code under the workflow-wide `-D warnings` -- passes on one lane and
fails on the other, for a reason unrelated to the diff. The pin keeps the
two test lanes one compiler. It shields the queue from nothing else, and
nothing here does: `lint (windows)` compiles current stable on both events
and is required by the aggregate, so a stable that adds a lint between a
pull request's green and its enqueue ejects the entry through that leg,
exactly as before the hosted lane existed. The pin moves when the image is
re-curated, in the same change. Measured,
`MUT-TEST-WINDOWS-TOOLCHAIN-FLOATS` and
`MUT-TEST-WINDOWS-TOOLCHAIN-BEHIND-THE-IMAGE`.

## `pub(super) const TEST_WINDOWS_TOOLCHAIN_COMPONENTS: &str = "clippy";`

The components the hosted lane installs, exactly. `clippy` is a test
dependency of this job as it is of `test`: the effect-denial fixtures drive
`clippy-driver`, and the action installs the minimal profile. Measured,
`MUT-TEST-WINDOWS-COMPONENTS-WITHOUT-CLIPPY`.

## `pub(super) const TEST_WINDOWS_PLATFORM: &str = "windows-latest";`

The [`CI_TARGETS`] runner whose tests run in [`TEST_WINDOWS_JOB`] rather than
in the `test` matrix -- and, on the queue lane, the runner that job runs on.
Its shell and cfg valuations carry over to the guest: the image carries
PowerShell 7, so a `run:` step resolves to `pwsh` there exactly as on
`windows-latest`, and it builds the same MSVC tuple.

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

The aggregate's script, pinned. Its loop reads `TEST_COVERAGE` since the
partition: the coverage job is a gate like any other, and a lane whose shards
did not run the whole suite fails `upstroke-ci` through it. Measured,
`MUT-AGGREGATE-COVERAGE-DROPPED` and `MUT-AGGREGATE-COVERAGE-UNREAD`.

A pin rather than a family of substring predicates, which is what the
standing row forbids growing more of. The enumerated escape is
`for gate in LINT LINT_WINDOWS MSRV TEST; do : LINT_MACOS` -- a loop that
omits a gate while a `contains` check for the omitted name still passes --
and a pin refuses it outright. The pin is not its own oracle: the loop's gate
list is re-derived from `needs:` below and compared, so this literal being a
faithful copy is itself checked against the job graph.

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
aggregate are the two that run the suite, the hosted test step and the
Windows step, and their maps are pinned whole by [`TEST_STEP_ENV`] and
[`TEST_WINDOWS_STEP_ENV`] rather than admitted by field name.

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
not, and the guest's NTFS does.

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

## `pub(super) const TEST_WINDOWS_STEP_ENV: [(&str, &str); 1] = [(TEMP_FOLDS_CASE_KEY, "1")];`

The same declaration on both Windows suite steps, `1` outright: NTFS folds
case on the guest and on `windows-latest` alike. The macOS shard step carries
[`TEST_STEP_ENV`], the whole-suite step's map, for the same reason.
Measured, `MUT-TEST-WINDOWS-CASEFOLD-DECLARATION-DROPPED`.

## `pub(super) const TEST_STEP_FIELDS: [&str; 7] =`

The fields a step that runs the suite may declare: [`STEP_FIELDS`] plus the
`env` whose map [`TEST_STEP_ENV`] and [`TEST_WINDOWS_STEP_ENV`] pin, and the
`if:` that says which legs run it -- pinned, per step, to [`WHOLE_SUITE_LANE`]
on the whole-suite step and [`HOSTED_LANE`] on the shard step. Every other
`run:` step of both test jobs keeps [`STEP_FIELDS`].

## `pub(super) const LANE_STEP_FIELDS: [&str; 4] = ["if", "name", "uses", "with"];`

The fields the hosted lane's toolchain install may declare: a `uses:` step's
fields plus the `if:` that names the lane. The condition skips the step on
the guest, not its archive: the runner resolves and downloads every `uses:`
during job setup before any `if:` is read, so the guest lane depends on
GitHub serving this archive as it already depends on it serving
`actions/checkout`, and a download failure fails the job before the skipped
step. That availability dependency is the price of one job for both lanes. The one place in this contract
`if:` is admitted on a step, and admitted for that step alone: on the step
that runs the suite it is a job that reports success having run nothing on
the other lane, and on a `run:` step it is a script one lane executes and
the pull-request lane -- the one a reviewer watches -- never shows. No
`run:` in the set, so a lane-conditional step cannot be a script at all.
Measured, `MUT-TEST-WINDOWS-SUITE-ON-ONE-LANE` and
`MUT-TEST-WINDOWS-HOSTED-ONLY-RETARGET`. The partition admits the same set
on one more `uses:` step, the upload of a shard's record, where the `if:` is
pinned to [`HOSTED_LANE`]: under `always()` a failed shard would upload a
record of a run that did not finish. The two suite steps admit `if:` through
[`TEST_STEP_FIELDS`], each pinned to its own condition.

## `pub(super) const STRATEGY_FIELDS: [&str; 2] = ["fail-fast", "matrix"];`

The fields of the two test jobs' `strategy:`. `max-parallel:` is a limit
this contract does not model; `fail-fast:` must be `false`, so one leg's
failure never cancels the others before they report. Measured,
`MUT-TEST-WINDOWS-FAIL-FAST`.

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

## `pub(super) const TEST_WINDOWS_JOB_FIELDS: [&str; 5] =`

The fields the Windows test job declares: the `test` job's set, `strategy:`
included since the partition made its legs a matrix. `if:` and
`continue-on-error:` are absent by construction, as everywhere in this
contract.

## `pub(super) const COVERAGE_JOB: &str = "test-coverage";`

The job that makes the partition safe. A shard's green says only that the
tests its filters selected passed; this job runs
`.github/scripts/partition.py verify` over the records every partitioned leg
uploaded and refuses the run unless, for each group the event's lane
partitions, the shards' harnesses listed one universe, each selected exactly
what the table's predicate selects from it, the selections are disjoint and
cover it, and every target's run accounts for its selection with no failure.
The aggregate requires it like any gate. Measured,
`MUT-COVERAGE-JOB-DELETED`.

## `pub(super) const COVERAGE_JOB_NAME: &str = "partition coverage";`

Its name, which does not begin `test (`: it runs no test, and nothing that
counts test legs by name should count it.

## `pub(super) const COVERAGE_JOB_FIELDS: [&str; 5] =`

Its fields, as an equality. `if:` is absent so that a failed, cancelled or
skipped leg skips it and the aggregate reads `skipped`; `continue-on-error:`
is absent so a refusal cannot report success. Measured,
`MUT-COVERAGE-RUNS-ALWAYS` and `MUT-COVERAGE-ABSOLVED`.

## `pub(super) const COVERAGE_NEEDS: [&str; 2] = ["test", TEST_WINDOWS_JOB];`

Every job that runs a partitioned leg. Measured,
`MUT-COVERAGE-NEEDS-NARROWED`.

## `pub(super) const COVERAGE_RUNNER: &str = "ubuntu-latest";`

Where the verifier runs.

## `pub(super) const COVERAGE_DOWNLOAD_INPUTS: [(&str, &str); 2] = [`

The download's inputs, whole: this run's artifacts by the shard records'
prefix, into the directory the verifier reads. No `run-id` and no token, so
no other run's records can stand in for this run's legs. Measured,
`MUT-COVERAGE-OTHER-RUN`.

## `pub(super) const COVERAGE_COMMAND: &str =`

The verifier's command, character for character, run from the checkout: the
code that judges the legs is the candidate's checked-out
`.github/scripts/partition.py`, never anything the legs uploaded. Measured,
`MUT-COVERAGE-VERIFIER-ECHOED` and `MUT-COVERAGE-RUNS-A-DOWNLOADED-SCRIPT`.

## `pub(super) const PARTITION_TABLE: &str = ".github/scripts/partition.table";`

The predicate table the runner and the verifier read.

## `pub(super) const WINDOWS_GROUP: &str = "windows";`

The table's group of hosted Windows shards, which the Windows matrix runs on
the lanes the table partitions it on.

## `pub(super) const MACOS_GROUP: &str = "macos";`

The table's group of macOS shards, which the `test` matrix runs on every
lane, so the table must partition it on every lane.

## `pub(super) const PARTITION_EVENTS: [&str; 3] = ["merge_group", "pull_request", "push"];`

The events `ci.yml` triggers on, each of which the table must give a lane.

## `pub(super) const PARTITION_TABLE_LINES: [&str; 12] = [`

Every line of the table that is not blank or a comment, pinned in order. The
table decides which tests each partitioned leg runs and which legs the
verifier expects, so a change to it is a change to this contract, made in the
same commit; the two test matrices are derived from these lines rather than
written down a second time.

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
test leg's hosted lane pins [`GOLDEN_IMAGE_TOOLCHAIN`], the image's version,
for the opposite reason: that leg must match the guest, not lead it.
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
