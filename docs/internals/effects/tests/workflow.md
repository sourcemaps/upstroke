# `src/effects/tests/workflow.rs`

Extended notes for [`src/effects/tests/workflow.rs`](../../../../src/effects/tests/workflow.rs).

The code is the authority for what it does. These notes started as the module's source prose.
Each code fragment in a heading is an exact source substring. When a heading names an enclosing
item before `›`, find that item first, then the following fragment within it.

## Module

The CI workflow's structural oracle: what is wrong with `ci.yml`, and the
mutations that prove each complaint fires.

Every claim here is an equality over a parsed mapping or an exact scalar
pin, never a `contains` over text — `BRIDGE-CI-SHAPE-TEST-IS-A-SUBSTRING-ORACLE`
is the row that records what a substring reading of this surface misses.
[`WORKFLOW_ESCAPES`] is the other half: each entry is a document that *does*
the forbidden thing, and a complaint **code** it must be refused as. A
refusal for an unrelated reason is not a refusal of that escape.

The shape being checked against is `super::ci_model`'s, not a second copy of
it. The `#[test]` wrappers that drive these functions stay in `super`: this
module is the oracle, not the harness, and every name in it is deliberately
not a test name.

The three effect denials are **restored** here rather than inherited.
`super`'s module-level allowance exists because that file drives
`clippy-driver` over fixtures it has to create; this module reads two files
and writes none, so the allowance has no business reaching it.

## `pub(super) fn ci_workflow_text() -> String {`

`.github/workflows/ci.yml`, with CRLF collapsed.

The collapse is not cosmetic. This test runs on `windows-latest`, where the
checkout can land CRLF line endings, and every claim below is an equality
against an exact scalar. Normalising here makes the oracle read the same
document on all three runners instead of depending on the scanner's line-break
handling -- the platform-shaped half of a mutation is the half that is only
ever measured on Linux.

## `pub(super) fn parse_workflow(text: &str) -> Result<Yaml, String> {`

Parse a workflow as YAML 1.2, refusing duplicate keys.

Duplicate-key rejection is a property of the parser this crate depends on for
exactly this reason, and
[`the_workflow_parser_rejects_duplicate_keys_and_reads_on_as_a_string`]
executes it: under last-one-wins every equality below reads the winning entry
and a mutation hides in the loser.

## `pub(super) fn field<'a>(node: &'a Yaml, key: &str) -> Option<&'a Yaml> {`

The value of `key` in a mapping node.

## `pub(super) fn field_names(node: &Yaml) -> BTreeSet<String> {`

Every key of a mapping node. A non-string key is rendered rather than
dropped, so `on:` read as a boolean by a YAML 1.1 parser would show up here
as an unexpected field rather than as a silent absence.

## `fn scalar_set(node: &Yaml) -> Option<BTreeSet<String>> {`

A sequence of scalars as a set, or `None` if the node is not that.

## `fn scalar_map(node: &Yaml) -> Option<BTreeMap<String, String>> {`

A mapping of scalar to scalar, or `None` if the node is not that.

## `fn gate_stem(job: &str) -> String {`

The environment variable the aggregate's loop reads for `job`.

## `fn field_complaints(node: &Yaml, required: &[&str], optional: &[&str]) -> Vec<String> {`

Every field the contract requires but the node does not declare, and every
field it declares that the contract does not know about.

## `fn defaults_shell<'a>(node: &'a Yaml, where_: &str, out: &mut Vec<String>) -> Option<&'a str> {`

The `shell:` a `defaults:` mapping sets, and every way its shape is wrong.

## `fn effective_shell(`

The shell a `run:` step actually executes under.

GitHub resolves it step, then job `defaults.run.shell`, then workflow
`defaults.run.shell`, then the runner's platform default. Reading only the
step is how a workflow-level default silently swaps the interpreter under
every gate at once -- measured, `MUT-WORKFLOW-DEFAULT-SHELL-SWAPPED`.

## `fn shell_complaints(`

Complain unless `step` resolves to exactly `expected`, and unless that is a
shell GitHub defines rather than a command template it will run instead.

## `if field(step, "shell").is_some() && scalar(step, "shell").is_none() {`

A `shell:` key whose value is not a string resolves to nothing at all
here, and this contract would then read the platform default and pass.
YAML reads the bare word `true` as a boolean, so that is not hypothetical.

## `fn stem_collisions(jobs: &BTreeSet<String>) -> BTreeMap<String, Vec<String>> {`

The job ids whose gate stems collide.

`gate_stem` upper-cases and turns `-` into `_`, so `lint-windows` and
`lint_windows` are one variable name. Every collection built from stems is a
map or a set, so a collision **collapses** rather than duplicating: the env
mapping loses an entry, the loop's expected set loses a member, and both
equalities below compare a shorter list against a shorter list and pass. The
collision is therefore checked before anything is derived from the stems.
Measured, `MUT-AGGREGATE-STEM-COLLISION`.

## `fn runner_target(runner: &str) -> Option<&'static CiTarget> {`

The [`CI_TARGETS`] row whose `runner` is `runner`: the platform whose default
shell a job on that runner gets. A gate or an `msrv` entry checked from
another platform's runner resolves its shell for that runner, so the macOS
gate on `ubuntu-latest` is held to Linux's `bash`.

## `pub(super) fn cross_target(command: &str) -> Option<&str> {`

The word after `--target` in a command, if it names one: the tuple a gate
compiles when it is not its runner's own.

## `fn ci_gate_complaints(doc: &Yaml) -> Vec<String> {`

Every way the parsed workflow fails the gate-wiring contract.

One function so the same contract runs against the real document and against
each mutation in [`WORKFLOW_ESCAPES`]; an oracle only ever run on conforming
input is an oracle nobody has seen refuse anything. Each complaint opens with
a `[kebab-code]`, and the escape table names the code it must provoke -- so a
mutation that fails for an unrelated reason does not count as refused.

A target's gate is exactly one job whose `runs-on:` is the target's
`check_runner` and one of whose steps runs the target's `clippy_gate`, and
the three gates are three different jobs; `[no-gate-job]` otherwise, naming
both. The pair, not the runner alone: `lint (macos)` and `lint` both run on
`ubuntu-latest`, and only the `--target aarch64-apple-darwin` on the first
tells them apart. Measured, `MUT-GATE-ECHOED`, `MUT-GATE-RUNNER-DRIFT` (the
macOS gate's job moved to `windows-latest`) and
`MUT-GATE-CROSS-TARGET-DROPPED` (the flag dropped: two jobs then match the
Linux gate and none the macOS one).

Where the gate's command carries `--target <triple>`, the job's toolchain
step must install exactly that tuple through `targets:`, or the gate is
refused as `[gate-toolchain-target]`. Without the input the job fails for want
of the standard library, which is loud; the pin keeps the tuple in one place
with the command that names it. Measured, `MUT-GATE-TOOLCHAIN-TARGET-DROPPED`.

## `fn ci_gate_complaints(doc: &Yaml) -> Vec<String>` › `if scalar(step, "run").is_some() {`

Only `run:` steps have a shell; a `uses:` step runs an action. The shell is
resolved for the gate job's runner, the target's `check_runner`, not for the
runner its test legs use.

## `fn aggregate_complaints(doc: &Yaml, jobs: &Yaml, job_names: &BTreeSet<String>) -> Vec<String> {`

Every way the aggregate fails to make each gate a required one.

## `fn aggregate_complaints(doc: &Yaml, jobs: &Yaml, job_names: &BTreeSet<String>) -> Vec<String> {` › `let mut expected_needs: BTreeSet<String> = job_names.clone();`

The `needs` set is DERIVED: every job but the aggregate itself. A job that
is not needed cannot fail the aggregate, so adding one and forgetting to
wire it is the same defect as dropping one, and this equality refuses both.

## `fn aggregate_complaints(doc: &Yaml, jobs: &Yaml, job_names: &BTreeSet<String>) -> Vec<String> {` › `let collisions = stem_collisions(&expected_needs);`

Before ANY collection is derived from the stems. Two job ids that
normalise to one variable name collapse every set and map built below, and
a collapsed expectation compares equal to a collapsed reality.

## `fn aggregate_complaints(doc: &Yaml, jobs: &Yaml, job_names: &BTreeSet<String>) -> Vec<String> {` › `let on_ubuntu = CI_TARGETS`

The aggregate runs on `ubuntu-latest`, and its script is bash. The check
is on the RESOLVED shell, not on the declaration: a workflow-level
`defaults.run.shell` reaches this step too, and a custom shell would let
the required check pass without reading a single gate result.

## `fn aggregate_complaints(doc: &Yaml, jobs: &Yaml, job_names: &BTreeSet<String>) -> Vec<String> {` › `let env = field(step, "env").and_then(scalar_map);`

The binding, not its existence. `LINT_MACOS_RESULT: ${{ needs.lint-windows
.result }}` is a copy-paste that satisfies any existence check, reads a
passing sibling, and reports the required context green over a red leaf.
Measured: an earlier version of this assertion accepted exactly that.

## `fn aggregate_complaints(doc: &Yaml, jobs: &Yaml, job_names: &BTreeSet<String>) -> Vec<String> {` › `let expected_stems: BTreeSet<String> = wired.iter().map(|job| gate_stem(job)).collect();`

Re-derived from `needs`, so the pin above is checked against the job graph
rather than trusted as a copy. `for gate in LINT LINT_WINDOWS MSRV
TEST_LINUX TEST_MACOS TEST_WINDOWS_HOSTED; do : LINT_MACOS` is the
enumerated escape: it satisfies a search for the omitted name while the loop
never reads it. The Linux and macOS test jobs are two gates since
2026-11-24, `test-linux` and `test-macos`, read as `TEST_LINUX_RESULT` and
`TEST_MACOS_RESULT` where the one `test` job was `TEST_RESULT`; nothing here
names them, since `needs`, the bindings and the loop are all derived from
the job graph. Measured, `MUT-AGGREGATE-DROPS-TEST-MACOS`, under
`aggregate-needs`.

## `pub(super) fn ci_test_job_complaints(doc: &Yaml) -> Vec<String> {`

Every way the jobs that execute this file's fixtures on Linux and macOS fail
their contract. Each row of [`TEST_JOBS`] is checked against its own job,
with the same checks, and every complaint names the job it is about. Until
2026-11-24 one `test` job ran both platforms' legs; the codes are the ones
that job's checks used, where what they refuse is unchanged.

The predecessor read the job's text with comments stripped and asked whether
the word `clippy` appeared on a `components:` line and whether the file
contained the test command anywhere. Both are satisfied by an `echo`, and the
strip existed only because the job's nine-line comment spelled the needle --
`PR4-CENSUS-COMMENT-ORACLE` in the test whose purpose is to answer "which
command runs this?". A parsed document has no comments in it at all, so that
class is gone by construction rather than by a strip whose bite had to be
asserted.

## `pub(super) fn ci_test_job_complaints(doc: &Yaml) -> Vec<String> {` › `if field(jobs, RETIRED_TEST_JOB).is_some() {`

A job named `test` ([`RETIRED_TEST_JOB`]) left in the workflow, code
`test-job-split`. The split is the contract: beside the two jobs such a job
runs legs no row holds to anything, and in their place it runs both
platforms' legs under one `runs-on:` this function no longer reads. No row
of the ledger measures it; the rows that measured the one job's mistakes
were re-aimed at the two.

## `pub(super) fn ci_test_job_complaints(doc: &Yaml) -> Vec<String> {` › `let Some(job) = field(jobs, id) else {`

A row whose job is missing is one complaint naming the job's id, code
`test-job-missing`, and the other row is still checked: a missing
`test-macos` does not hide what is wrong with `test-linux`.

## `pub(super) fn ci_test_job_complaints(doc: &Yaml) -> Vec<String> {` › `let runs_the_suite = scalar(step, "run") == Some(TEST_SHARD_COMMAND);`

The suite step is the one whose `run:` is [`TEST_SHARD_COMMAND`], the
command with the leg's `matrix.tests` handed to libtest, and it is recognised
by that text everywhere this function asks for it: for the step's field set
and case-fold map here, for the count of exactly one suite step
(`test-job-command`), and for the capture's place immediately after it
([`capture_complaints`]). A step that ran the bare [`TEST_COMMAND`] instead
would be an unpinned script, and the count of suite steps would be zero; so
would one whose line named a filter of its own where the pinned text reads
the leg's `matrix.tests`. Measured, `MUT-CI-CARGO-TEST-STEP-DELETED`,
`MUT-TEST-SHARD-SELECTION-DROPPED` and `MUT-TEST-FILTER-HARDCODED`.

`MUT-TEST-FILTER-HARDCODED` writes its line as a double-quoted scalar,
`- run: "cargo test --all-targets --all-features -- engine::"`. A plain
scalar cannot end in a colon. Measured on 2026-11-17 with the row's
replacement unquoted: the parser refused the document, `mapping values are
not allowed in this context`, and the row failed as refused under `parse`
and not under `test-job-command`. Quoted, the step's script is the cargo
line with `engine::` in the selection's place and nothing else.

The step that runs the suite, and the host acceptance capture step after it
([`HOST_ACCEPTANCE_CAPTURE`]), are the two steps in each job that may carry
an `env:`, and only the map the job's row pins (`step_env`): `1` on
`test-macos`, the empty string on `test-linux`. That map is the
declaration that the leg's temporary directory folds case, under which
`the_temporary_object_scan_resolves_case_aliases_as_the_filesystem_does`
requires its native branch. The capture repeats it so the record is made
under the suite's environment. Every other step keeps [`STEP_FIELDS`], and
each map is compared whole by [`step_env_complaints`], the capture's under
its own code, `test-capture-step-env`.

Measured for the suite step: `MUT-TEST-CASEFOLD-DECLARATION-DROPPED`,
`MUT-TEST-CASEFOLD-DECLARED-ON-THE-WRONG-LEG` and
`MUT-TEST-STEP-RETARGETED-THROUGH-THE-ADMITTED-ENV`. Measured for the
capture: `MUT-TEST-CAPTURE-CASEFOLD-DROPPED` and
`MUT-TEST-CAPTURE-RETARGETED-THROUGH-THE-ADMITTED-ENV`.

The suite step's three rows anchor on its `run:` line together with the
`env:` lines. The capture step repeats the same `env:` text, and
[`mutate_workflow`] measures nothing unless an anchor occurs exactly once in
its job. Each row's mutation, target and code are what they were before the
capture existed. The `run:` line gained ` -- ${{ matrix.tests }}` when the
macOS suite was divided, and the three rows were re-spelled with it; nothing
else in them moved. At the split each `env:` line became its job's literal
and the rows were re-spelled with it: the dropped declaration and the
retargeting key on `test-macos`, and the wrong-leg row on `test-linux`,
whose `""` it makes `"1"`, the declaration made on the platform that does
not fold case. Under one job that row moved the declaration from the macOS
legs to the ubuntu ones in one expression; one mutation touches one job,
and the nearer of its two halves is the loud one.

The container capability observation after the capture
([`CAPABILITY_OBSERVATION`]) is the third step that may carry an `env:`, and
only the same map, under its own code `test-capability-step-env`. Its field
set is [`CAPABILITY_STEP_FIELDS`], not [`TEST_STEP_FIELDS`]: exactly `env`,
`name`, `run` and `timeout-minutes`. [`observation_step_complaints`] holds the
bound to [`CAPABILITY_STEP_TIMEOUT_MINUTES`] under `test-capability-step`.
Measured, `MUT-TEST-CAPABILITY-CASEFOLD-DROPPED`,
`MUT-TEST-CAPABILITY-DOCKER-HOST-INJECTED`,
`MUT-TEST-CAPABILITY-REQUIRE-DOCKER-INJECTED`,
`MUT-TEST-CAPABILITY-TIMEOUT-DROPPED` and
`MUT-TEST-CAPABILITY-TIMEOUT-WIDENED`.

The observation's rows anchor on its `name:` line, its `timeout-minutes: 4`
line, or its own script lines. Each of those occurs once in each job.

Its script spells `set -eo pipefail` rather than the capture's bare
`set -o pipefail` line, because that line is the anchor
`MUT-TEST-CAPTURE-EXIT-SWALLOWED` requires exactly once in its job. Every
earlier row's anchor, mutation and code are unchanged.

At the split the rows that named the one `test` job were aimed at one of the
two: the observation's and the capture's at `test-macos`, whose `env:` they
spell with `"1"`, except `MUT-TEST-CAPTURE-STEP-DELETED`, which spells
`test-linux`'s `""`; the suite step's, the checkout's, the toolchain's and
the job fields' mostly at `test-linux`. The two jobs have the same steps, so
each such row measures the one check both jobs run.

## `pub(super) fn ci_test_job_complaints(doc: &Yaml) -> Vec<String> {` › `if scalar(step, "run").is_some() {`

Each `run:` step resolves its shell under the row's runner, the one machine
its job runs on, and must be that platform's default: a workflow-level
default swaps every one of them at once, which is the mutation the
step-only reading could not see. While one job ran both platforms' legs
each step was resolved once per runner that hosts tests; each job now has
one runner, and a runner this contract does not model is refused once, as
`step-shell`, before the steps are read.

## `pub(super) fn ci_test_job_complaints(doc: &Yaml) -> Vec<String> {` › `out.extend(test_leg_matrix_complaints(job, row));`

The matrix is the platform half of the same claim: a fixture that runs on
one platform proves nothing about the other. Each job's matrix is held to
the legs its row derives ([`test_leg_matrix_complaints`]), and the table's
runners, once for both jobs, to the same derived runner set the Clippy legs
are compared against, less the one platform whose suite runs in its own job
([`test_jobs_table_complaints`]).

## `pub(super) fn ci_test_job_complaints(doc: &Yaml) -> Vec<String> {` › `if scalar(job, "runs-on") != Some(row.runner) {`

The job's `runs-on:` is the row's runner as a literal. On another runner the
job runs that platform's shards elsewhere: every leg keeps its name and its
filters and is green, and no test of it ran on the platform it is named
for. An expression or a label set is a runner this contract cannot read.
Code `test-job-matrix`, the code the one job's `runs-on:` check had when it
required `${{ matrix.os }}`. Measured, `MUT-TEST-MACOS-ON-LINUX`, and
`MUT-CI-CARGO-TEST-STEP-SKIPPED` anchors on `test-linux`'s line.

## `pub(super) fn ci_test_job_complaints(doc: &Yaml) -> Vec<String> {` › `if scalar(job, "name") != Some(TEST_JOB_NAME) {`

The job's `name:`, as the workflow writes it ([`TEST_JOB_NAME`]). Nothing
reads a leg's name but a person and the API's listing, and that is the
reason to hold it: the name is how a run tells its legs apart, and under a
name that does not read `matrix.leg` -- the runner, as the one job's name
read `matrix.os` until the macOS suite was divided -- a job's shards publish
one name. Code `test-job-name`. Measured, `MUT-TEST-LEGS-NAMED-BY-RUNNER`,
which names `test-macos` `test (macos-latest)`.

## `pub(super) fn ci_test_job_complaints(doc: &Yaml) -> Vec<String> {` › `let minutes = field(job, "timeout-minutes").and_then(Yaml::as_i64);`

The job's limit, exactly [`TEST_TIMEOUT_MINUTES`] and an integer: one number
for every leg of both jobs. An expression, such as the one the one job
carried while the macOS suite ran whole, 45 minutes on macOS and 30 on
ubuntu, reads as a string here and is refused as no number at all. Code
`test-job-timeout`. Measured, `MUT-TEST-TIMEOUT-RAISED-FOR-MACOS` and
`MUT-TEST-TIMEOUT-RAISED`: 45 on `test-macos` and 45 on `test-linux`, each
a plain integer, which reads as a number and is refused by the equality
alone. With a job per platform the macOS raise needs no expression.

## `pub(super) fn ci_test_job_complaints(doc: &Yaml) -> Vec<String> {` › `let toolchains: Vec<&Yaml> = steps_of(job)`

`clippy` is a TEST dependency of this job, not only a lint one:
`every_declared_effect_denial_refuses_for_the_reason_it_declares` drives
`clippy-driver` over one fixture per resolution shape, and
`dtolnay/rust-toolchain` installs the minimal profile. Measured, mutation
`MUT-CI-STOPS-INSTALLING-CLIPPY`.

## `fn hosts_tests(target: &CiTarget) -> bool {`

Whether a [`CI_TARGETS`] runner hosts its platform's tests in a job of
[`TEST_JOBS`] -- every platform but the one whose suite runs in its own
shards' job.

## `fn test_jobs_table_complaints() -> Vec<String> {`

The runners of [`TEST_JOBS`] are, in order and one job each, the
[`CI_TARGETS`] runners that host tests ([`hosts_tests`]). Code
`test-job-matrix`, checked once for both jobs, and it reads no workflow.
Each job's matrix is held to its row, so a platform dropped from the table
and from `ci.yml` together would satisfy every check of the jobs that are
left; the table is therefore held to the platform list the Clippy legs and
the MSRV leg are derived from, and a platform cannot leave the test jobs
that way. Until the split this was the first of
[`test_leg_matrix_complaints`]' five checks, over `TEST_LEGS`, the one
job's table of runners and filters.

## `struct TestLeg {`

One leg of a test job's matrix as the oracle compares it: the leg's label
and the libtest arguments. The expected legs and the legs read from
`ci.yml` are both made of it, as the Windows shards' are of [`ShardLeg`], so
the comparison is one equality and a refusal prints both sides field by
field. Until the split it also carried the leg's `os:`, its runner; the
runner is now the job's.

## `fn test_legs(row: &TestJob) -> Vec<TestLeg> {`

The table a test job's matrix must equal, derived and never written out:
one leg for each string [`shard_selections`] makes of the row's filters, in
order.

- `tests` is the string. A runner with no filter has one, the empty string:
  no arguments, the whole suite.
- `leg` is the runner alone where it has that one leg, and otherwise the
  runner, a comma and the shard's place, `k of n`. The count is the number
  of strings, so a filter added to a runner's list renames that runner's
  legs and no label is written down.

Today that is two legs for `test-linux`, `ubuntu-latest, 1 of 2` and
`ubuntu-latest, 2 of 2`, and five for `test-macos`, `macos-latest, 1 of 5` to
`macos-latest, 5 of 5`. The label is what GitHub puts in the leg's job name
in place of `${{ matrix.leg }}` ([`TEST_JOB_NAME`]), so each check name is
the one the leg had in the one job. No runner has an empty
filter list today, so no leg is labelled by its runner alone: the Linux leg
was, as `test (ubuntu-latest)`, the name it had while the matrix was an
`os:` axis, until its suite was divided in two.

## `fn test_leg(entry: &Yaml) -> Option<TestLeg> {`

One `include:` entry of `ci.yml` read as a leg, or nothing: a mapping with
exactly the keys `leg` and `tests`, each a string. A third key, an `os:`
among them, is refused rather than ignored, for [`shard_leg`]'s reason: an
`include:` entry may set anything a step or the job reads from `matrix`,
and a key this contract does not model is one whose reader it has not
looked for. A
`tests: ""`, which the Linux leg carried while it ran the suite whole, is a
string, the empty one, and reads as a leg; a bare `tests:` is YAML's null,
which is no string, and does not.

## `fn test_leg_matrix_complaints(job: &Yaml, row: &TestJob) -> Vec<String> {`

A test job's `strategy:`, whole, under the one code `test-job-matrix`, each
complaint naming the job. Four things, each its own complaint:

1. `strategy:` declares exactly `fail-fast` and `matrix`. `max-parallel` is
   the rest of what a strategy may say today; an equality over the field set
   refuses it and whatever GitHub adds next.
2. `fail-fast` is `false`. Otherwise the first leg to fail cancels the legs
   still running, and a run reports one shard's failures and none of the
   others'.
3. `matrix:` declares exactly `include`. An axis beside the list changes
   what the list means, by the rule [`windows_shard_matrix_complaints`]
   records for the Windows shards, and `exclude:` removes combinations of an
   axis. The legs are the list and nothing else.
4. `include:` is, in order, exactly [`test_legs`]' entries for the row,
   each read by [`test_leg`].

The table's runners are held to the platforms once, for both jobs, by
[`test_jobs_table_complaints`]; until the split that was this function's
first check, and the fourth check here was its fifth.

Until the macOS suite was divided the matrix was one `os:` axis, compared as
a set with the runners that host tests, and `include:` was refused with
`exclude:` as a key that could add a platform nobody declared. The legs
became the `include:` list because an axis cannot say them: the seven
shared two runners and differed in what they are handed. The platform
claim the set equality made is [`test_jobs_table_complaints`] and each
job's fourth check together.

In order, though GitHub does not run the legs in order: within a runner the
order is the shard table's, each leg skipping the filters of the legs before
it, and an equality over an ordered list is the plain way to hold every leg
to its own row.

These legs hold no count of passing tests to a floor, where each Windows
shard does, and the fourth check is what stands in for one here. A shard's
floor refuses a leg whose filters matched almost nothing; in these jobs such
a leg is green, and the tests its filters no longer match are run by a later
leg, since every later leg only skips those filters. That holds while every
leg's arguments are its runner's table's, which is this equality.

Measured, `MUT-TEST-MATRIX-EXCLUDED`, `MUT-CI-TEST-MATRIX-NARROWED`,
`MUT-TEST-MATRIX-KEEPS-WINDOWS`, `MUT-TEST-MACOS-SHARD-NARROWED`,
`MUT-TEST-MACOS-LAST-SHARD-SKIPS-MORE` and `MUT-TEST-UBUNTU-LEG-FILTERED`.
The first three were written for the `os:` axis and were re-spelled for the
list: an `exclude:` beside `include:`, the Linux entries deleted, and a
Windows entry added. Of an `include:` list GitHub documents that its entries
are processed after `exclude:`, so by that rule the first removes no leg
today, and the row says so; it is refused as a key the contract does not
read. The second deleted the one Linux leg while that leg ran the suite
whole. The Linux suite is two legs now and the row deletes both, so that it
still removes a platform and not a shard of one. Its
anchor therefore spells the two Linux legs' arguments, `engine::` and
`--skip engine::`, which the oracle otherwise writes out in one test; the
macOS five's are written out in that test and nowhere else.

At the split the rows were aimed at the job that holds what they mutate,
and the entries lost their `os:` lines: the exclusion names the first macOS
leg in `test-macos`; the narrowing deletes `test-linux`'s two entries, four
lines; and the Windows entry is a third entry of `test-linux` labelled
`windows-latest`, which would run the suite whole on `ubuntu-latest` under a
Windows name. Every anchor that spelled a `- os:` line now spells the
`- leg:` line that opens the entry.

The last three change what a leg is handed, and the fourth check refuses
each. `MUT-TEST-MACOS-SHARD-NARROWED` puts a `--skip effects::` first in the
last macOS leg's arguments and `MUT-TEST-MACOS-LAST-SHARD-SKIPS-MORE` puts
it last, after every word the table derives. The appended skip's anchor is
the end of the last entry's `tests:` value and the `steps:` line after it,
so it spells that leg's last skip and not its whole arguments, and the
sentence above still holds of the macOS shards.
`MUT-TEST-UBUNTU-LEG-FILTERED` was written for the one Linux leg, whose
`tests:` was the empty string, and gave it the filter `engine::`: the leg
that ran the suite whole ran a subset. That leg is gone, and the row is now
its nearest equivalent. It gives the last Linux leg, the one with no filter,
the first leg's arguments, `engine::` where it read `--skip engine::`: both
Linux legs run the one subset, and the tests that run on Linux alone run
nowhere.

Three more rows make three of the Windows shards' mutations on the macOS
legs, and the fourth check refuses each. `MUT-TEST-MACOS-FILTER-DROPPED-FROM-ITS-LEG`
deletes the second leg's first filter,
`engine::topology::coordinator::tests::interleaving::`, from that leg's own
`tests:` alone; every later leg still skips it, so the coordinator's
interleaving tests run on no macOS leg. `MUT-TEST-MACOS-SKIP-DROPPED`
deletes the last leg's first skip, so the tests the first leg runs under
`...coordinator::tests::a_p` run in the last leg too. And
`MUT-TEST-MACOS-FILTER-MOVED-BETWEEN-LEGS` moves the third leg's last
filter, `events::log::`, to the front of the fourth leg's, which still
skips it: libtest's skip wins over any filter, and those tests run on no
macOS leg. These legs hold no count to a floor, so under each of the three
every leg is green. The moved filter's anchor spells the third leg's skips
whole and the opening of the fourth leg's value, so a change to the table
re-spells it; the other two spell a leg's opening words.

A deleted macOS leg and the table check were measured by hand instead, on
2026-11-17, while the macOS legs were three and one `test` job ran them, each
in the tree and then restored. With the third macOS entry's three lines
removed from `ci.yml`, three tests of `super` failed, each with the
`include:` complaint and both tables printed --
`the_workflow_shape_oracle_refuses_every_escape_the_ledger_names` at its
clean-workflow check,
`the_workflow_that_runs_these_tests_installs_the_compiler_they_need` and
`every_platform_this_crate_configures_for_has_a_clippy_gate_the_aggregate_requires`.
With `macos-latest` removed from `TEST_LEGS`, the one job's table, and its
three entries from `ci.yml` together, the same three failed with the table
complaint, now [`test_jobs_table_complaints`]', and no other. Neither was
measured again at the split.

## `fn installs_a_toolchain(step: &Yaml) -> bool {`

Whether a step is the toolchain action, by its `uses:` prefix.
[`ci_test_windows_hosted_job_complaints`] reads the version and the
components from the steps it selects. It was written for the guest's job,
the one job that could carry no such step at all.

## `pub(super) fn ci_test_windows_hosted_job_complaints(doc: &Yaml) -> Vec<String> {`

Every way the job that runs the Windows suite fails its contract. It is the
shape of [`ci_test_job_complaints`], on one runner and under PowerShell:

- the field set, [`TEST_WINDOWS_HOSTED_JOB_FIELDS`]: the five of `test-linux` and `test-macos`,
  with no `needs:` and no `if:`;
- the name, the runner and the timeout;
- the shard matrix, whole ([`windows_shard_matrix_complaints`]);
- every step's fields, the case-fold map on the suite, capture and
  observation steps, and the observation's bound;
- the platform-default shell on every `run:` step;
- exactly one suite step running [`WINDOWS_SHARD_WITNESS`], the capture
  immediately after it and the observation immediately after that;
- one input-free checkout at step 0;
- every script and action on the job's pins,
  [`TEST_WINDOWS_HOSTED_SCRIPTS`];
- one toolchain install, at step 1, of [`GOLDEN_IMAGE_TOOLCHAIN`], with
  `clippy` among its components;
- no cache step.

The job is found by its id, [`TEST_WINDOWS_HOSTED_JOB`]. Every pin on the
Windows suite hangs from that id, so a workflow without the job is refused
under `test-windows-hosted-missing` before anything else here is read. A
renamed job is not a false green, since the aggregate's derived `needs:`
refuses the renamed workflow too; the code is there so that the contract
notices the name going before it notices nothing else. Measured,
`MUT-TEST-WINDOWS-HOSTED-RENAMED-AWAY`.

The runner is [`TEST_WINDOWS_PLATFORM`], as a scalar, under
`test-windows-hosted-runner`: the suite runs on GitHub's runner on every
event. A label set names a self-hosted machine, and an expression a runner
this contract cannot read. Measured, `MUT-TEST-WINDOWS-HOSTED-ON-THE-GUEST`:
the fourteen legs put on the labels of the guest deleted on 2026-11-20, which no
runner carries, so that every leg queues for a machine that does not exist.

The timeout is exactly [`TEST_WINDOWS_HOSTED_TIMEOUT_MINUTES`], an integer,
under `test-windows-hosted-timeout`. 40 minutes a leg is 1.75 times the
slowest leg that ended in run 43719719561 and inside the merge queue's 60. A
leg that prints every result line and does not end holds the required
check, and the queue entry behind it, until that limit
([`TEST_WINDOWS_HOSTED_JOB`]). Measured,
`MUT-TEST-WINDOWS-HOSTED-TIMEOUT-RAISED`.

A `needs:` or an `if:` on the job is refused by the field set, under
`unexpected-job-field`. Until 2026-11-20 both were required. Each of the two
Windows suite jobs, the guest's and this one, needed the picker and ran for
one of its two answers, and a helper the two checks shared,
`windows_home_complaints`, held `needs:`, the condition and the timeout.
With one job, an `if:` is a Windows suite some event does not run. Measured,
`MUT-TEST-WINDOWS-HOSTED-CONDITIONED`, a condition that excludes the merge
queue. It is not a false green either: on a queue entry the job is skipped,
the aggregate reads `skipped` and fails, and no entry lands. The oracle
refuses it before it is pushed, where the queue would refuse it one entry at
a time.

The toolchain action is required at step 1, by [`toolchain_complaints`]
under `test-windows-hosted-toolchain`, so no step runs a compiler the runner
preinstalled. Until 2026-11-20 the guest's job was held to the opposite
rule, no install at any step, because the guest's image carried the
compiler. No row moves this job's install off step 1;
`MUT-TOOLCHAIN-NOT-FIRST` measures that position on `lint-windows`.

The steps are one list that every leg runs, so they are read once: nothing
in a step may vary by leg except through the three `matrix` expressions the
pinned suite script carries, and what those evaluate to is the matrix
check's to hold.

Measured, every row named `MUT-TEST-WINDOWS-HOSTED-*`, and three that mutate
this job under other names: `MUT-STEP-USES-UNPINNED`,
`MUT-DEFAULTS-WORKING-DIRECTORY` and
`MUT-JOB-PERMISSIONS-ON-THE-HOSTED-SHARDS`.

## `pub(super) fn ci_test_windows_hosted_job_complaints(doc: &Yaml) -> Vec<String> {` › `let runs_the_suite = scalar(step, "run") == Some(WINDOWS_SHARD_WITNESS);`

The suite step is the one whose `run:` is [`WINDOWS_SHARD_WITNESS`], and it
is recognised by that text everywhere this function asks for it: for the
step's field set and case-fold map here, for the count of suite steps, and
for the capture's place after it.

The suite step carries the same allowance as the Linux and macOS test jobs', pinned to
[`TEST_WINDOWS_STEP_ENV`] under `test-windows-hosted-step-env`: the hosted
runner's NTFS folds case, so the declaration is `1` outright. The capture
step after it ([`HOST_ACCEPTANCE_WINDOWS_CAPTURE`]) carries the same map
under `test-windows-hosted-capture-step-env`. Measured,
`MUT-TEST-WINDOWS-HOSTED-CASEFOLD-DECLARATION-DROPPED` and
`MUT-TEST-WINDOWS-HOSTED-CAPTURE-CASEFOLD-DROPPED`. Each row anchors on its
step's `name:` line together with the `env:` lines, because the three steps
repeat the same `env:` text and an anchor must occur once in the job.

The container capability observation ([`CAPABILITY_WINDOWS_OBSERVATION`])
takes the same map whole under `test-windows-hosted-capability-step-env`,
the field set [`CAPABILITY_STEP_FIELDS`], and the bound
[`CAPABILITY_STEP_TIMEOUT_MINUTES`] under
`test-windows-hosted-capability-step`
([`observation_step_complaints`]). Measured,
`MUT-TEST-WINDOWS-HOSTED-CAPABILITY-CASEFOLD-DROPPED` and
`MUT-TEST-WINDOWS-HOSTED-CAPABILITY-TIMEOUT-DROPPED`. Those two anchor on
the step's `timeout-minutes: 4` line, which occurs once in the job. A key
put beside the declaration is measured on `test-macos`' observation; the
row that put one on a Windows observation mutated the guest's job and went
with it on 2026-11-20.

## `pub(super) fn ci_test_windows_hosted_job_complaints(doc: &Yaml) -> Vec<String> {` › `&STEP_FIELDS`

No step of this job may carry an `if:`. The suite step and the capture keep
[`TEST_STEP_FIELDS`], the observation [`CAPABILITY_STEP_FIELDS`] and every
other step [`STEP_FIELDS`], and none admits one: on the step that runs the
suite a condition is a job that reports success having run nothing for the
events it excludes, and on any other step it is a script or an action one
event executes and the pull-request build -- the one a reviewer watches --
never shows. Both are refused by the field set before their values are
read, under `unexpected-step-field`. Measured,
`MUT-TEST-WINDOWS-HOSTED-SUITE-ON-ONE-LANE`: the suite step conditioned on
the merge queue, so that the queue still runs the suite and every pull
request is green on Windows having run none of its tests. The rows that
measured this on the guest's job went with it on 2026-11-20.

## `pub(super) fn ci_test_windows_hosted_job_complaints(doc: &Yaml) -> Vec<String> {` › `let running = steps_of(job)`

Exactly one suite step, under `test-windows-hosted-command`. The whole step,
not the commands inside it. The build and the harness loop say which suite
is built and run; the lines before them are what say which compiler builds
it, and the lines after them are what say the suite ran. An equality over
the step is what stops those lines being dropped, or the floor lowered to a
number an unexecuted shard clears. Measured,
`MUT-TEST-WINDOWS-HOSTED-COMPILER-CHECK-DROPPED`,
`MUT-TEST-WINDOWS-HOSTED-WITNESS-DROPPED` and
`MUT-TEST-WINDOWS-HOSTED-FLOOR-DROPPED`.

A step whose harness loop dropped the selection from the harnesses'
arguments, or named a filter of its own where the pinned text reads
`matrix.tests`, is an unpinned script here, and the count of suite steps is
zero. Until the legs reported their tests' times both rows edited the line
that started `cargo` over the selection; they edit the loop's line now, and
measure the same thing. Measured,
`MUT-TEST-WINDOWS-HOSTED-SHARD-SELECTION-DROPPED` and
`MUT-TEST-WINDOWS-HOSTED-FILTER-HARDCODED`. The second is the one no floor
refuses: every leg runs the tests whose names contain `engine::`, and by the
counts the ten-shard floors of 2026-11-20 were set from that subset clears
all fourteen floors. The first six of those ten shards and the
ninth, whose filters each contained `engine::`, were 802 tests, and the rest
of `engine::topology::`, which none of them named, was 484 tests by the
counts of 2026-11-19: 1,286, above the last leg's 985 and every other floor.
Before the re-split of 2026-11-20 the subset was the first seven shards'
tests, 1,295 by those counts, against a last floor of 734.

The same equality holds how the step runs the suite and what it does about a
test process left running. `MUT-TEST-WINDOWS-HOSTED-SUITE-PIPED-AGAIN` makes
line 7 pipe each harness into `Tee-Object`, as the step once piped Cargo,
and `MUT-TEST-WINDOWS-HOSTED-SURVIVOR-ALLOWED` removes line 11, the throw
for such a process.

It holds how the step builds the suite and times it as well, which is what
the build was split from the run for: Cargo builds once, with `--no-run`
and without `RUSTC_BOOTSTRAP`, and the step then runs each harness itself
with the variable set, which libtest needs before it reports a test's time
on a stable compiler. Why the variable must not reach the build is under
`WINDOWS_SHARD_WITNESS` in the CI model notes: three crates' build scripts
read it. One row for each way the step can lose that.

`MUT-TEST-WINDOWS-HOSTED-BUILD-RUNS-THE-SUITE` starts the build without
`--no-run`. Cargo then runs every harness over the whole suite, untimed,
before the loop runs the shard again; on 2026-11-15 the whole suite alone
took 3,311 to 4,292 s in the harness on this runner, more than a leg's 40
minutes.

`MUT-TEST-WINDOWS-HOSTED-BUILD-ERRORS-READ-UNGUARDED` puts back the line
after the build as it stood before it read the build's errors in a `try`:
`if ($code -ne 0) { Get-Content $err; exit $code }`. Under
`$ErrorActionPreference = 'stop'` a build that failed and whose errors file
cannot be read then ends the step with PowerShell's error, not Cargo's
code, and without the errors that say what failed. Not a false green, since
the step still fails, but the leg's log says a file could not be read and
not why the build failed.

`MUT-TEST-WINDOWS-HOSTED-EMPTY-HARNESS-LIST-ALLOWED` removes the refusal of
an empty list of harnesses. Not a false green while the lines after it
hold: with no harness run there is no output to read and no count to clear
the floor. The row is there so that a build whose messages name no harness
is refused for that, by a throw that says so.

`MUT-TEST-WINDOWS-HOSTED-HARNESS-TIMES-DROPPED` starts the harnesses
without `-Z unstable-options --report-time`. Every leg runs its shard,
clears its floor and is green, and no log carries a test's time: the times
the step exists to record are lost on all fourteen legs, and no exit code or
floor shows it.

`MUT-TEST-WINDOWS-HOSTED-LOOP-PASSES-A-FAILED-HARNESS` takes the loop's
`break` away, so a harness that passes after one that failed overwrites
`$code` with zero, `exit $code` does not fire, and the leg is judged by its
count, which the harnesses that passed can clear.

`MUT-TEST-WINDOWS-HOSTED-LOOP-DROPS-THE-FAILED-CODE` keeps the `break` and
drops the code: `$code` still holds the build's zero, with the same result.
Cargo's own `test` stops at the first harness that fails, and the loop is
held to doing the same and keeping that harness's code.

`MUT-TEST-WINDOWS-HOSTED-CODE-KEPT-AFTER-THE-COPIES` keeps a harness's code
only after the loop copies its two outputs. Under
`$ErrorActionPreference = 'stop'` a harness that exits 101 and whose
standard output cannot be read then ends the step on the copy's error,
which the copy's `catch` throws again because `$code` still holds the
build's zero or the last harness's. Not a false green, since the step still
fails, but the failing harness's code is never kept and its standard error
never printed.

`MUT-TEST-WINDOWS-HOSTED-OUTPUT-COPY-UNGUARDED` takes the `try` and `catch`
away from the copy of a harness's standard output: a `.part` file that
cannot be read ends the step on PowerShell's error, with the same result
for a harness that failed. The loop is held to keeping the code as soon as
`WaitForExit()` returns, and to a `try` for each copy whose `catch` throws
only for a harness that passed.

`MUT-TEST-WINDOWS-HOSTED-BOOTSTRAP-BEFORE-THE-BUILD` sets
`RUSTC_BOOTSTRAP` on a line of its own above the build, where the loop's
line still sets it again. The build then runs in bootstrap mode, and the
three build scripts turn on nightly features the other legs' builds leave
off, from a tree nobody changed.

`MUT-TEST-WINDOWS-HOSTED-BOOTSTRAP-ON-THE-BUILD-LINE` sets it on the
build's own line, ahead of the `Start-Process` that builds: the same build
in bootstrap mode, written where a reading that asked only whether a line
above the build sets the variable would pass it.

`MUT-TEST-WINDOWS-HOSTED-CARGO-AFTER-BOOTSTRAP` runs a `cargo` after the
variable is set: the doctests, which `--all-targets` leaves out, on a line
of their own after the loop. The step's process still holds the variable,
so that Cargo builds in bootstrap mode. A later step would not, since it
runs in a process of its own; this line is in the step that sets it.

Each row in this section edits the workflow alone and is refused under
`test-windows-hosted-command`. The same edit made to the workflow and to
[`WINDOWS_SHARD_WITNESS`] together is not this pin's to refuse:
`windows_suite_wait_complaints` in `super` reads those relations from the
constant, and `the_windows_leg_counts_the_tests_it_ran` holds its build
line, its loop line and its floor line to what the model makes them.

## `pub(super) fn ci_test_windows_hosted_job_complaints(doc: &Yaml) -> Vec<String> {` › `let selected = field(step, "with").and_then(|with| scalar(with, "toolchain"));`

The install selects [`GOLDEN_IMAGE_TOOLCHAIN`], exactly: the compiler the
guest's image carried when the guest was retired. Not `stable`, which
floats ahead of it, and not an older version, which omits what the pinned
compiler enables.

The pin is kept so that the suite's compiler moves by an edit -- the
install, the two lines of the suite step that refuse any other compiler and
the model's constant, together -- and not with GitHub's stable. Under
`stable` a test whose outcome depends on the compiler -- a version-gated
test, a std behaviour change, a new lint in test code under `-D warnings`
-- moves with GitHub's image, and `lint (windows)` already compiles,
code-generates and links the tree on current stable on every event. While
the guest also ran the suite, the reason was the guest: a second compiler
here made such a test's result depend on which home the picker chose.

With the suite step intact either drift throws at the compiler questions on
every run. This refuses it before it is pushed, as the drift between the two
places the workflow writes the version; edited together with the suite step
the drift is refused there, as an unpinned script. The check is this job's
own rather than [`toolchain_complaints`]'s `expected`, whose complaint is
written for the jobs that pin `stable`. Measured,
`MUT-TEST-WINDOWS-HOSTED-TOOLCHAIN-FLOATS` and
`MUT-TEST-WINDOWS-HOSTED-TOOLCHAIN-DOWNGRADED`.

## `pub(super) fn ci_test_windows_hosted_job_complaints(doc: &Yaml) -> Vec<String> {` › `if scalar(step, "uses").is_some_and(|uses| uses.starts_with(CACHE_ACTION)) {`

No cache step, at any position. The job carries none, and the reason its
cache step was taken out has gone: until 2026-11-20 `master` saved a cache for this
job only on a push the picker sent here, so nearly every run would have
uploaded a cache nothing later read. Every push to `master` now runs these
legs, so `master` would save one that pull requests and queue entries could
restore. Whether that pays inside a leg's 40 minutes is not measured, and
adding a cache step is a change of its own, made together with this refusal
([`CACHE_ACTION`]). The capture's placement rule already refuses a cache
step between the suite and the capture; this refuses one anywhere, and
names the cause. Measured, `MUT-TEST-WINDOWS-HOSTED-CACHE-RESTORED`.

## `pub(super) fn ci_test_windows_hosted_job_complaints(doc: &Yaml) -> Vec<String> {` › `let components: BTreeSet<&str> = steps_of(job)`

The `clippy` component, read from the install step the way the Linux and
macOS test jobs' check reads it: the suite's effect-denial fixtures run `clippy-driver`, and
`dtolnay/rust-toolchain` installs the minimal profile. Measured,
`MUT-TEST-WINDOWS-HOSTED-WITHOUT-CLIPPY`.

## `struct ShardLeg {`

One leg of the Windows shards' matrix as the oracle compares it: the shard's
label, the libtest arguments, and the floor. The expected legs and the legs
read from `ci.yml` are both made of it, so the comparison is one equality
and a refusal prints both sides field by field.

## `pub(super) fn windows_shard_labels() -> Vec<String> {`

The `shard:` value of each leg, in order: `1 of 14` to `14 of 14`. The count
is [`WINDOWS_SHARD_FILTERS`]' length and one more, as [`shard_selections`]
makes one more shard than it is given filter lists, so a list added to the
table renames every leg and no label is written down.

The label is what GitHub puts in the leg's job name, in place of
`${{ matrix.shard }}`, and what the suite step prints beside its count.
[`windows_shard_legs`] is its one reader now. Until 2026-11-20 the picker's
fixtures in `super` built the legs' listed names from it as well.

## `fn windows_shard_legs() -> Vec<ShardLeg> {`

The table the job's matrix must equal, derived and never written out: leg
`k` has the `k`-th label, the `k`-th string [`shard_selections`] makes of
[`WINDOWS_SHARD_FILTERS`], and the `k`-th of [`WINDOWS_SHARD_FLOORS`].
`the_shard_tables_run_every_test_in_exactly_one_shard` holds the floors to
one for each shard, so the three lists are one length.

## `fn shard_leg(entry: &Yaml) -> Option<ShardLeg> {`

One `include:` entry of `ci.yml` read as a leg, or nothing: a mapping with
exactly the keys `shard`, `tests` and `floor`, the first two strings and the
third an integer. A fourth key is refused rather than ignored. An
`include:` entry may set anything a step or the job reads from `matrix`,
and a key this contract does not model is one whose reader it has not
looked for. A quoted `floor: "24"` is refused too. GitHub would put the
same text into the script, so nothing would run differently; the refusal
keeps the table to one spelling, a floor that can only be a number.

## `fn windows_shard_matrix_complaints(job: &Yaml) -> Vec<String> {`

The `strategy:` of [`TEST_WINDOWS_HOSTED_JOB`], whole, under the one code
`test-windows-hosted-matrix`. Four things, each its own complaint:

1. `strategy:` declares exactly `fail-fast` and `matrix`.
2. `fail-fast` is `false`. Otherwise the first leg to fail cancels the legs
   still running, and a run reports one shard's failures and none of the
   others'.
3. `matrix:` declares exactly `include`. An axis beside the list changes
   what the list means. GitHub documents that each `include:` entry is added
   to every combination the axes make where it overwrites none of their
   values, a later entry overwriting what an earlier one added, and becomes
   a leg of its own only where it cannot be added. By that rule, with one
   axis of one value all fourteen entries land on one combination and the last
   wins: one leg, the last shard's, and a job whose result is `success`.
   That is the rule as documented; no workflow was run to watch it.
4. `include:` is, in order, exactly [`windows_shard_legs`]' fourteen, each read
   by [`shard_leg`].

The fourth is the one the shard table exists for. The rule that every test
runs in exactly one shard is proved of [`shard_selections`]' strings; it
says nothing of a workflow whose strings differ from them. A leg dropped
from the list, or one whose arguments select less than the table's, runs
its tests in no leg while the others are green, and the aggregate reads
one `success`. A leg whose arguments select more runs another shard's tests
twice. A floor below the table's is one a shard clears having executed
almost none of its tests; the script's pin cannot see that, since the
script reads the floor from the matrix ([`WINDOWS_SHARD_WITNESS`]).

In order, though GitHub does not run the legs in order: the order is the
table's, each leg skipping the filters of the legs before it, and an
equality over an ordered list is the plain way to hold every leg to its own
row.

Measured, for the second check, `MUT-TEST-WINDOWS-HOSTED-FAIL-FAST-ENABLED`.

Measured, for the third, `MUT-TEST-WINDOWS-HOSTED-MATRIX-AXIS-ADDED` and
`MUT-TEST-WINDOWS-HOSTED-SHARD-EXCLUDED`: an axis beside the list, and an
`exclude:` beside it. Of a list with no axis GitHub documents that its
entries are processed after `exclude:`, so by that rule the second removes
no leg today, and the row says so; it is refused as a key the contract does
not read, as `MUT-TEST-MATRIX-EXCLUDED` is in `test-macos`.

Measured, for the fourth:

- `MUT-TEST-WINDOWS-HOSTED-SHARD-DROPPED`, the second leg's entry deleted;
- `MUT-TEST-WINDOWS-HOSTED-SHARD-NARROWED` and
  `MUT-TEST-WINDOWS-HOSTED-LAST-SHARD-SKIPS-MORE`, a `--skip effects::` put
  first in the last leg's arguments and put last, after every word the
  table derives;
- `MUT-TEST-WINDOWS-HOSTED-SHARD-FLOOR-LOWERED` and
  `MUT-TEST-WINDOWS-HOSTED-SHARD-FLOOR-ZEROED`, the last leg's floor at 1
  and at 0. At 1 the leg must still execute a test; at 0 it is green having
  executed none;
- `MUT-TEST-WINDOWS-HOSTED-FILTER-DROPPED-FROM-ITS-LEG`, the second leg's
  first filter deleted from its own `tests:` and kept in every later leg's
  skips, so the one test it names runs on no leg;
- `MUT-TEST-WINDOWS-HOSTED-SKIP-DROPPED`, the last leg's first skip deleted,
  so the tests it excluded run in the first leg and again in the last;
- `MUT-TEST-WINDOWS-HOSTED-FILTER-MOVED-BETWEEN-LEGS`, the third leg's last
  filter moved to the front of the fourth leg's, which still skips it:
  libtest's skip wins, and its tests run on no leg;
- `MUT-TEST-WINDOWS-HOSTED-FILTER-WIDENED`, the first leg's
  `...coordinator::tests::a_h` cut to `...tests::a_` on its own line, so
  that leg also runs tests later legs run;
- `MUT-TEST-WINDOWS-HOSTED-FILTERS-COMMA-JOINED`, the first leg's first two
  filters joined by a comma: libtest reads the word as one filter, which no
  test's name contains.

Two of the last five only raise a leg's count, which no floor refuses. The
other three lower one leg's count by the tests they lose, which its floor
refuses only where they take the leg below it: a floor counts tests, not
which ones. What refuses all five is the equality of each leg's `tests:`
with the string `shard_selections` derives, word for word.

The deleted leg's anchor is that entry's three lines, so the second leg's
arguments are written out there, the only place they are. A change to the
table has to re-spell that
row; until it does, the row fails saying the workflow moved. The appended
skip's anchor is the end of the last entry's `tests:` value and its `floor:`
line, which spells that leg's last skip and not its whole arguments.

The change from five shards to ten, on 2026-11-19, re-spelled each of these
rows against the new table and kept what each measures: the second leg
became the coordinator's tests named `a_r...`, the last leg's last skip
`workspace_manager::` and its floor 734, and the `exclude:` names
`2 of 10`. The re-split of 2026-11-20 kept the second leg, so the deleted
leg's anchor and the `exclude:` stand as they were, and moved the last
leg: its last skip is now `engine::tests::` and its floor is 965, and the
appended skip's row and the two floor rows are re-spelled with them. No row
of the then single `test` job's was re-spelled for the macOS table then: none spells a
macOS leg's whole arguments, and the last macOS leg's last skip was still
`runner::`. `MUT-TEST-WINDOWS-HOSTED-FILTER-HARDCODED` kept its anchor and
had its count restated
([`ci_test_windows_hosted_job_complaints`]).

The fourteen shards of 2026-11-22, each named by a list of filters,
re-spelled them again. The second leg's entry is now the one whose own
filters open with `effects::tests::every_pr6_...` and include the
coordinator's `...tests::a_p`, with floor 36; the last leg's last skip is
`engine::tests::a_d` and its floor 969; the narrowed leg and the `exclude:`
name `14 of 14` and `2 of 14`. On the macOS side
`MUT-TEST-MACOS-LAST-SHARD-SKIPS-MORE` now anchors on the last macOS leg's
last skip, `engine::tests::t`. `MUT-TEST-WINDOWS-HOSTED-FILTER-HARDCODED`
kept its anchor and had its count restated against the fourteen floors.

The re-pack from three timed runs re-spelled them once more. The second
leg's entry still opens with `effects::tests::every_pr6_...` and goes on with
`engine::topology::recover::tests::a_k`, `...recover::tests::an`,
`engine::tests::a_p` and `runner::contract::`, with floor 43; the last leg's
last skip is `engine::tests::a_f` and its floor 980; and the last macOS
leg's last skip is `engine::tests::an`. `MUT-TEST-WINDOWS-HOSTED-FILTER-HARDCODED`
kept its anchor and had the last floor restated. The five Windows rows the
re-pack added spell short pieces of the first four legs' `tests:` values;
the moved filter's spells the third leg's skips whole, from its last filter
to the opening of the fourth leg's value, so a change to the table re-spells
it too.

The rebalance from the five runs' medians, which moved whole filters
between legs, re-spelled them once more. The second leg's entry now ends
its own filters with the coordinator's `...tests::o`, moved there from the
first leg, with floor 44; the moved filter's row spells the third leg's
skips, which now end with that `o`, and floor 224; the first leg's first two
filters, which the comma-joined row joins, are now `...tests::a_h` and
`...tests::t`; and the last leg's last skip is `engine::tests::f` and its
floor 985. On the macOS side the last leg's last skip is
`engine::topology::attempt::`, and the third leg's last filter, which
`MUT-TEST-MACOS-FILTER-MOVED-BETWEEN-LEGS` moves, is `events::log::`. The
second macOS leg now names `effects::tests::`, so this oracle's own tests no
longer run in the last macOS leg; the `--skip effects::` of the two macOS
skip rows now takes the tests under `topology::effects::` outside its
`tests` module, such as `topology::effects::registry::`'s, which still run
there.
`MUT-TEST-WINDOWS-HOSTED-FILTER-HARDCODED` kept its anchor and had the last
floor restated.

Before that row a deleted leg was measured by hand, on 2026-11-17, while
the legs were five, in `ci.yml` itself and then restored: with the fifth
entry's three lines removed, three tests of `super` failed, each with the
fourth complaint and both tables printed --
`the_workflow_shape_oracle_refuses_every_escape_the_ledger_names` at its
clean-workflow check,
`the_hosted_windows_shards_run_the_suite_on_the_compiler_they_install`
under the name it then had, and
`every_platform_this_crate_configures_for_has_a_clippy_gate_the_aggregate_requires`.
The same three failed the same way with the second leg's `--skip` naming
`tests::b`.

## `fn capture_complaints(`

The native HOST acceptance record's step: exactly one step of the job runs the
pinned capture script, and it is the step immediately after the one step that
runs the pinned suite. Codes: `test-capture-command` and
`test-windows-hosted-capture-command`.

This check is what refuses an omitted capture. Without it, nothing in this
contract asks for the record, and deleting the step leaves every other check
green. A capture whose script was edited in any way is no longer the pinned
script, so it counts as missing. That covers:

- filtered to another test or to none;
- `--ignored` dropped, so libtest reports the test ignored and Cargo exits
  zero;
- echoed;
- its exit propagation, its zero-test refusal or its frame check removed.

[`step_pin_complaints`] also refuses such a script as unpinned.

"Immediately after" because a step between the suite and the capture can
change what the capture runs while every pin still matches. A cache restore
can replace the harness the suite built and ran; a checkout can replace the
tree.

Measured, in the Linux and macOS test jobs, the first row in `test-linux`
and the rest in `test-macos`:

- `MUT-TEST-CAPTURE-STEP-DELETED`
- `MUT-TEST-CAPTURE-IGNORED-NOT-RUN`
- `MUT-TEST-CAPTURE-FILTER-MOVED`
- `MUT-TEST-CAPTURE-EXIT-SWALLOWED`
- `MUT-TEST-CAPTURE-ZERO-TESTS-ADMITTED`
- `MUT-TEST-CAPTURE-FRAMING-UNCHECKED`
- `MUT-TEST-CAPTURE-SEPARATED-FROM-THE-SUITE`

Measured, on the Windows shards:

- `MUT-TEST-WINDOWS-HOSTED-CAPTURE-STEP-DELETED`
- `MUT-TEST-WINDOWS-HOSTED-CAPTURE-EXIT-SWALLOWED`
- `MUT-TEST-WINDOWS-HOSTED-CAPTURE-SEPARATED-FROM-THE-WITNESS`
- `MUT-TEST-WINDOWS-HOSTED-CHECKOUT-TWICE`

The rows that deleted the Windows capture or edited its script mutated the
guest's job and went with it on 2026-11-20, and from then until 2026-11-21 no
row deleted or edited the shards' capture. The first two rows restore that,
each refused as `test-windows-hosted-capture-command`: the step deleted,
its name, `env:` and `run:`, and the capture's
`if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }` line removed. That line
also stands in the capability step, so the second row's anchor is the line
together with the capture's `cargo test` line before it, and its
replacement keeps the `cargo test` line. The other edits, a moved filter,
`--ignored` dropped, the zero-test refusal or the frame check removed, are
measured on `test-macos`' capture alone.

The last two measure the position guard: a pinned cache step, and a second
input-free checkout, between the witness and the capture. The checkout is
refused here and by [`checkout_complaints`], and its row is measured under
that function's code.

A field on the capture beyond the admitted set — `if:` or
`continue-on-error:` — is refused as `unexpected-step-field`
(`MUT-TEST-CAPTURE-STEP-DISABLED` and `MUT-TEST-CAPTURE-STEP-ABSOLVED`, in
`test-macos`).

Three controls each disabled one of the new checks; each check is
load-bearing:

| Check disabled | Row no longer refused |
| --- | --- |
| this one | `MUT-TEST-CAPTURE-STEP-DELETED` |
| its position guard alone | `MUT-TEST-CAPTURE-SEPARATED-FROM-THE-SUITE` |
| the capture's env pin | `MUT-TEST-CAPTURE-CASEFOLD-DROPPED` |

Every earlier row stayed refused in all three runs.

## `fn observation_complaints(`

The container capability observation's step (G7, root direction `ecf15a9c`).
Exactly one step of the job runs the pinned observation script, and it is the
step immediately after the one step that runs the pinned host acceptance
capture. Codes: `test-capability-command` and
`test-windows-hosted-capability-command`.

As with [`capture_complaints`], this check is what refuses an omitted
observation, and an edited script counts as a missing one:

- filtered to another test, to none, or widened by dropping `--exact`;
- `--ignored` dropped;
- echoed;
- its exit propagation, its zero-test refusal or its frame check removed.

"Immediately after the capture" has three reasons. A step between them could
replace the harness or the tree. The contract places the observation
success-only after the HOST capture. And a step above the capture would run
before the record that step makes.

Measured, in `test-macos`:

- `MUT-TEST-CAPABILITY-STEP-DELETED`
- `MUT-TEST-CAPABILITY-IGNORED-NOT-RUN`
- `MUT-TEST-CAPABILITY-FILTER-MOVED`
- `MUT-TEST-CAPABILITY-FILTER-WIDENED`
- `MUT-TEST-CAPABILITY-ECHOED`
- `MUT-TEST-CAPABILITY-EXIT-SWALLOWED`
- `MUT-TEST-CAPABILITY-ZERO-TESTS-ADMITTED`
- `MUT-TEST-CAPABILITY-FRAMING-UNCHECKED`
- `MUT-TEST-CAPABILITY-SEPARATED-FROM-THE-CAPTURE`
- `MUT-TEST-CAPABILITY-ABOVE-THE-CAPTURE`

`MUT-TEST-CAPABILITY-ABOVE-THE-CAPTURE` is a true move. Both scripts still
match their pins and each runs once, in swapped order. Its anchor spans the
capture step through the observation step.

Measured, on the Windows shards:

- `MUT-TEST-WINDOWS-HOSTED-CAPABILITY-STEP-DELETED`

The rows that edited the Windows observation's script, or put a step between
it and the capture, mutated the guest's job and went with it on 2026-11-20;
no row does either to the shards' observation.

A conditional or failure-swallowing field on the observation, `if:` (a
`!cancelled()` expression included) or `continue-on-error:`, is refused as
`unexpected-step-field`. The rows are
`MUT-TEST-CAPABILITY-STEP-DISABLED`,
`MUT-TEST-CAPABILITY-STEP-RUN-WHEN-NOT-CANCELLED` and
`MUT-TEST-CAPABILITY-STEP-ABSOLVED`, in `test-macos`.

No earlier check was removed or loosened. [`TEST_STEP_FIELDS`] and
[`STEP_FIELDS`] are unchanged; the observation step alone may carry
`timeout-minutes`.

## `fn observation_step_complaints(step: &Yaml, named: &str, code: &str) -> Vec<String> {`

The observation step's own fields. They are exactly [`CAPABILITY_STEP_FIELDS`],
every one required, so a dropped `env:` or `timeout-minutes:` is refused
here as well as by the env pin. The bound must read as the integer
[`CAPABILITY_STEP_TIMEOUT_MINUTES`]. A string, another number or no bound is
refused. Codes: `test-capability-step` and
`test-windows-hosted-capability-step`. Measured,
`MUT-TEST-CAPABILITY-TIMEOUT-DROPPED`, `MUT-TEST-CAPABILITY-TIMEOUT-WIDENED`
and `MUT-TEST-WINDOWS-HOSTED-CAPABILITY-TIMEOUT-DROPPED`.

## `fn checkout_complaints(job: &Yaml, named: &str, code: &str) -> Vec<String> {`

Every way a job's checkout points its steps at a tree other than the head
under test, or is not where the steps assume it: exactly one checkout, at
step 0, with no inputs.

Count and position first. A pinned action placed above the checkout runs
against an empty workspace, or one it prepared itself, and a second
checkout later can replace the candidate while the first, input-free one
still matches; both pass a reading that checks only the inputs of whatever
checkouts it finds, and the position the toolchain check asserts for an
install is only meaningful if the checkout is the step before it. Both were
measured on the guest's job, by rows that went with it on 2026-11-20: a
cache step above the checkout, on a job that installed nothing and so had no
install whose position would refuse it, and a second checkout later. On
2026-11-21 a frontier pass found that no row reached this guard: deleting
it left all 138 rows green. Two rows on the hosted shards measure it now,
each refused as `test-windows-hosted-checkout`:

- `MUT-TEST-WINDOWS-HOSTED-CHECKOUT-NOT-FIRST`, the pinned cache step above
  the checkout, which then checks out at `[1]`;
- `MUT-TEST-WINDOWS-HOSTED-CHECKOUT-TWICE`, a second input-free checkout
  immediately above the capture, at `[0, 4]`.

Each was measured against a control the same day: with the guard disabled,
neither row was refused under this code, since neither checkout has inputs
and nothing else in this function speaks. Both were still refused under
other codes. The step above the checkout also moves the install off step 1
(`test-windows-hosted-toolchain`) and is a cache step
(`test-windows-hosted-cache`); every job left that checks out installs a
toolchain at step 1, so a step above any of their checkouts is refused by
that position as well. The second checkout stands between the witness and
the capture (`test-windows-hosted-capture-command`).

`actions/checkout` with no inputs checks out the event's own ref: for a
pull request, the candidate merged onto its base. Any input -- `ref:`,
`repository:`, `path:` -- selects something else, and `ref: master` there
tests `master` while every other leg reads the candidate. [`STEP_FIELDS`]
admits `with:` because the toolchain and cache actions need it; on a
checkout step it is refused whole. Measured,
`MUT-TEST-WINDOWS-HOSTED-CHECKOUT-REF`, `MUT-TEST-CHECKOUT-REF` and
`MUT-WITNESS-CHECKOUT-REF`.

## `fn step_env_complaints(`

Every way a step's `env:` is not the map pinned for it -- absent, a different
value, an extra key, or a value YAML does not read as a string. An equality
over the whole map, for the reason the aggregate's is: a step-level
environment can retarget the compile the step performs, so admitting the
field by name would hand back the escape [`STEP_FIELDS`] closes.

## `fn step_pin_complaints(job: &Yaml, named: &str, code: &str, scripts: &[&str]) -> Vec<String> {`

Every way a step of a modelled job is something this contract did not pin.

The field and shell checks say how a step runs; this says what. A `run:`
step whose script is not in the job's pinned set can move the checkout --
`git fetch origin master && git checkout --detach FETCH_HEAD` -- before the
pinned command runs, so that command runs against another tree while the
runner, fields, shell and input-free checkout all still match. A `uses:`
step off [`PINNED_ACTIONS`] is code nobody here reviewed, with a checkout
of its own. Measured, `MUT-TEST-WINDOWS-HOSTED-RUN-RETARGETED`,
`MUT-TEST-RUN-RETARGETED`, `MUT-GATE-RUN-RETARGETED`,
`MUT-MSRV-RUN-RETARGETED` and `MUT-STEP-USES-UNPINNED`.

## `fn step_pin_complaints(job: &Yaml, named: &str, code: &str, scripts: &[&str]) -> Vec<String> {` › `let Some((_, allowed)) = ACTION_INPUTS`

The commit says which code runs; the inputs say what it is told to
do. `rust-cache`'s `cmd-format` wraps the commands it runs, so one
input on an allowlisted action at a pinned commit is enough to put
`git checkout` in front of every gate in the job.

## `fn step_pin_complaints(job: &Yaml, named: &str, code: &str, scripts: &[&str]) -> Vec<String> {` › `if uses.starts_with(TOOLCHAIN_ACTION) {`

The values, not only the key names. The toolchain action builds shell
text from `components` and `targets` and interpolates it into a Bash line,
so an allowlisted key with an unpinned value is a command the candidate
chose running inside an action this contract calls pinned. `targets` is held
to [`TOOLCHAIN_TARGETS`] here, on every job; which of those values a job
must give is pinned by the job's own contract.

## `fn toolchain_complaints(`

Every way a job installs a compiler other than the pinned one.

[`PINNED_ACTIONS`] pins the action by commit; this pins the input that
decides which compiler it installs. A gate downgraded from `stable` to the
version the Windows shards pin leaves no leg compiling on current stable,
and every other pin in this contract still matches. Two jobs are asked for
the count and the position of their install and not for its version. The
MSRV leg pins its own floor against the manifest, in
[`ci_msrv_job_complaints`], and the Windows suite job its own compiler, in
[`ci_test_windows_hosted_job_complaints`]. Measured,
`MUT-GATE-TOOLCHAIN-DOWNGRADED`.

## `out.push(format!(`

Zero installs is a job whose compiler something other than this workflow
chose. Every job this helper is asked about installs one. Until 2026-11-20
one job that compiled was not asked: the guest's, whose compiler its image
chose.

## `if *install != 1 {`

Position, not merely presence. The action installs a toolchain and makes
it the rustup default, so a step above it runs whatever the runner image
happened to preinstall -- during a release rollout that is the previous
stable, and the witness that exists to compile *current* stable would be
compiling something else while every other pin still matched.

## `fn workflow_env_complaints(doc: &Yaml) -> Vec<String> {`

Every way the workflow's own `env:` is not the pinned mapping.

The whole map, as an equality. A guard per name can only refuse names
somebody thought of, and the dangerous ones are the others: a Cargo target
runner bound here makes `cargo test` build every harness and run none, on
one target, which no other platform's leg would notice. The Windows legs'
suite step no longer runs its harnesses through `cargo test`: it builds
with `--no-run` and starts them itself, which a runner does not reach. The
two capture steps after it still run `cargo test`. Refusing the map
wholesale refuses that class rather than that instance.
Measured, `MUT-WORKFLOW-ENV-TARGET-RUNNER`.

## `fn workflow_permissions_complaints(doc: &Yaml) -> Vec<String> {`

Every way the workflow's `permissions:` is not the pinned mapping.

The whole map, as an equality, for the reason the `env:` map is pinned
whole: the field's presence is not the property that matters, its value is.
A widened token is available to every build script and test the candidate
ships, with the checkout's credential still configured, and discarding the
runner after the job recalls nothing it pushed.
Measured, `MUT-WORKFLOW-PERMISSIONS-WIDENED`.

## `fn job_permissions_complaints(doc: &Yaml) -> Vec<String> {`

No job declares `permissions:`. A block on a job replaces the workflow's for
that job, and hands whatever it names to a job that runs the candidate's
build scripts and tests. Every modelled job's field set already refuses the
key; this scan reaches a job added tomorrow that has no field set yet, and
names the cause. Code `job-permissions`.

With the picker gone there is no exception. Until 2026-11-20 the scan was
part of the picker's check and passed over that one job, which widened the
workflow's token by one read scope, to list other runs' jobs, and checked
out nothing. Every job is scanned now.
Measured, `MUT-JOB-PERMISSIONS-ON-THE-HOSTED-SHARDS`.

## `pub(super) fn ci_windows_build_witness_complaints(doc: &Yaml) -> Vec<String> {`

Every way the Windows codegen witness on current stable fails its contract.

The Windows suite's shards build with the pinned
[`GOLDEN_IMAGE_TOOLCHAIN`], which moves only by an edit to that pin. `cargo
check` and Clippy on `windows-latest` type-check current stable and stop
before codegen, so without this witness nothing on GitHub's current stable
ever code-generates or links the Windows tree: a Windows-only codegen or
link failure there would pass every other leg while the shards, on the
pinned compiler, link and pass. [`WINDOWS_BUILD_WITNESS`] builds every test
binary and executes none. It lives in the Windows Clippy gate's job, whose
field set and shells the gate contract pins; this pins the command, exactly
once, on exactly one `windows-latest` job. Measured,
`MUT-WINDOWS-BUILD-WITNESS-*`.

## `pub(super) fn ci_windows_build_witness_complaints(doc: &Yaml) -> Vec<String> {` › `if !steps_of(job)`

The carrier must be the Windows Clippy gate's job: that job's fields,
shells, scripts and checkout are pinned by the gate contract, so the
witness inherits every one of those pins. A witness on some other
`windows-latest` job would be a pinned command on an unpinned job.

## `pub(super) fn declared_rust_version() -> String {`

`Cargo.toml`'s `[package] rust-version`, as it is written there.

## `pub(super) fn declared_msrv_toolchain() -> String {`

The toolchain name the MSRV leg must install, derived from the manifest.

Derived rather than transcribed: a literal `"1.85.0"` here would make this
section its own oracle for the one fact it exists to hold, and a bump to
`rust-version` would leave the leg checking a floor the crate no longer
publishes.

The pin is exact, which `.github/scripts/test-docs-consistency.sh`'s C2 is
deliberately not: C2 accepts `rust-version` "or a patch release of it", so it
reads `toolchain: 1.85` as agreement. `dtolnay/rust-toolchain` resolves a
two-component name to the newest patch in the series, which is not the
`cargo +1.85.0` that `CODING_STANDARDS.md` §2, `CONTRIBUTING.md` and
`CLAUDE.md` all publish.

## `pub(super) fn three_component(version: &str) -> String {`

`1.85` as the toolchain name `1.85.0`; anything else unchanged.

Unchanged rather than repaired. A manifest value this does not understand
must reach the equality below and fail there with both strings quoted, not be
normalised into agreement with whatever the workflow happens to say.

## `pub(super) fn ci_msrv_job_complaints(doc: &Yaml) -> Vec<String> {`

Every way the MSRV leg fails to check the floor this crate publishes.

Nothing above this function reaches that job. [`ci_gate_complaints`] selects
a job by its `runs-on:` *and* a step whose `run:` is a target's Clippy gate,
and `msrv` matches neither -- it runs on `${{ matrix.os }}` and it runs
`cargo check`. So until this existed the only structural claim on the MSRV
leg was that the aggregate needs a job with that id: its matrix could be
narrowed to one runner or hollowed out with `exclude:`, its command could
lose `--locked` or become an `echo`, and its step could be absolved, with
every check in this section still passing.

One claim here is held elsewhere too, and its neighbour states its own
limits. `.github/scripts/test-docs-consistency.sh` C2 compares the toolchain
scalar with `rust-version` by grepping a text block; that file's `WITHDRAWN,
DELIBERATELY` note records that the gate makes "NO claim about which cargo
commands CI runs, whether CI executes them", because a command "can be
present and skipped (`if: false`)". A parsed document is what lets that claim
come back as an equality -- the same trade the rest of this section made, and
the reason `MUT-CI-CARGO-TEST-STEP-SKIPPED` is already a kill here rather
than history.

## `pub(super) fn ci_msrv_job_complaints(doc: &Yaml) -> Vec<String> {` › `if scalar(step, "run").is_some() {`

A matrix job, so each `run:` step resolves a shell once per runner its
entries name -- the distinct `check_runner`s of [`CI_TARGETS`], `ubuntu-latest`
and `windows-latest` -- and a workflow-level default reaches every entry at
once.

## `pub(super) fn ci_msrv_job_complaints(doc: &Yaml) -> Vec<String> {` › `out.extend(toolchain_complaints(job, MSRV_JOB, "msrv-toolchain", None));`

Position only: this leg installs the manifest's floor, not `stable`, and
the value is checked against `Cargo.toml` above.

## `fn msrv_entry(entry: &Yaml) -> Option<(String, String)> {`

One `include:` entry as its `(os, target)` pair, or `None` where it has keys
other than exactly `os` and `target` or a value that is not a string.

## `pub(super) fn ci_msrv_job_complaints(doc: &Yaml) -> Vec<String> {` › `let expected_entries: BTreeSet<(String, String)> = CI_TARGETS`

The platform half. The job checks the floor for three targets, one
`include:` entry apiece, and the entries are compared against the pairs
`(check_runner, triple)` derived from [`CI_TARGETS`], each once -- the same
table the Clippy gates and the test jobs' rows are derived from. The macOS
entry runs on `ubuntu-latest` and names its tuple, for the reason
`lint (macos)` does: GitHub runs at most five macOS jobs at once for the
account, each run asked for seven, and run 45175187088 showed each macOS job
starting as another ended. A floor is a per-target fact: a dependency that
raises its MSRV behind a `cfg`, or a call newer than the floor inside a
`#[cfg(target_os = "macos")]` region, fails on that target only.

`matrix:` holds `include:` alone; an axis or `exclude:` beside it changes which
combinations run, so either is refused as `[msrv-job-matrix]` whatever it
would do. A dropped entry, an entry retargeted to another's tuple and an entry
twice are refused the same way. `runs-on:` stays `${{ matrix.os }}`, and the
toolchain step's `targets:` is `${{ matrix.target }}`, the tuple the check
names with `--target`; a missing or other `targets:` is
`[msrv-job-toolchain]`. Measured, `MUT-MSRV-MATRIX-NARROWED`,
`MUT-MSRV-MATRIX-EXCLUDED`, `MUT-MSRV-CROSS-TARGET-DROPPED`,
`MUT-MSRV-TOOLCHAIN-TARGET-DROPPED` and, for the command's `--target`,
`MUT-MSRV-COMMAND-UNTARGETED`.

## `pub(super) fn ci_msrv_job_complaints(doc: &Yaml) -> Vec<String> {` › `let install_at = steps_of(job).iter().position(|step| {`

Order, not merely presence. `dtolnay/rust-toolchain` selects the toolchain
for the steps that FOLLOW it, so a check above it compiles on whatever the
runner image preinstalled -- stable -- while both steps are present, both
are exact, and every equality above passes. Measured,
`MUT-MSRV-CHECK-BEFORE-TOOLCHAIN`.

The install step is located by the toolchain it installs, not merely by the
action it uses: an install of something other than the derived floor is
already refused above, and pairing the order claim to the same exact value
keeps the two from drifting apart.

## `pub(super) fn rustflags_complaints(doc: &Yaml) -> Vec<String> {`

Every way the workflow-scope `-D warnings` fails to reach a compilation.

Two claims. The first is the pin: the workflow's own `env:` binds
[`RUSTFLAGS_KEY`] to exactly [`RUSTFLAGS_VALUE`]. The second is what makes
the first *effective*: no job and no step rebinds that name, and nothing
anywhere binds [`ENCODED_RUSTFLAGS_KEY`], which Cargo reads in preference to
it.

The override scan walks every job and every step rather than the jobs this
contract models, and that is why it is written separately from the field
sets. On today's document it is defence in depth -- `GATE_JOB_FIELDS`,
`TEST_JOB_FIELDS`, `MSRV_JOB_FIELDS`, `AGGREGATE_JOB_FIELDS` and
`STEP_FIELDS` already refuse an `env:` almost everywhere it could go. But the
`msrv` leg had no field set at all until this change, the aggregate's step
and the three suite-running steps are *allowed* an `env:`, and a job added
tomorrow has no field set until someone writes one. A rebinding anywhere is
refused by this scan on its own, which is what
`the_workflow_scope_rustflags_pin_refuses_weakening_and_every_override`
measures on documents the rest of the contract does not reach.

## `pub(super) fn rustflags_complaints(doc: &Yaml) -> Vec<String> {` › `for key in field_names(env) {`

Every other guarded binding at workflow scope, case-insensitively.
Two distinct defects share this arm: `CARGO_ENCODED_RUSTFLAGS`,
which Cargo reads instead of the pinned name, and a case variant of
`RUSTFLAGS` itself, which collides with the pinned line on Windows
and does nothing on Linux. The pinned key is skipped because the
match above already decided it.

## `fn guarded_env_key(key: &str) -> Option<&'static str> {`

The canonical guarded name `key` is, ignoring case, or `None`.

Whole-key equality, never a substring. `RUSTFLAGS_EXTRA`, `RUST_FLAGS` and
`CARGO_TERM_COLOR` are unrelated variables and a `contains` reading would
refuse bindings that never touch the warning policy.

Case-insensitive, because the environment on `windows-latest` is. GitHub
merges `env:` mappings by exact key and hands the result to the runner, which
sets them into a process environment where `rustflags` and `RUSTFLAGS` are one
variable. A lowercase job-level binding is therefore inert on Linux and
authoritative on Windows -- exactly the half of a mutation that only ever gets
measured on Linux, and the reason this comparison is not `==`.

## `fn env_name_tokens(script: &str) -> BTreeSet<String> {`

Every `[A-Za-z0-9_]` run of `script`, so a variable name is matched whole.

A `run:` scalar is an opaque script rather than a mapping, so the finest
granularity available over it is a token. Tokens are still enough to keep the
discipline the rest of this section keeps: `RUSTFLAGS_EXTRA` is one token and
is not `RUSTFLAGS`, which a `contains` reading could not tell apart.

## `fn writes_the_job_env_file(script: &str) -> bool {`

Whether `script` names the job-scoped environment file, in any of its forms.

`$GITHUB_ENV`, `${GITHUB_ENV}`, `$env:GITHUB_ENV`, `%GITHUB_ENV%` and
`${{ github.env }}` are one file, reached from bash, pwsh and cmd
respectively. A line written to it becomes an environment variable for every
later step of the same job.

## `fn rustflags_script_complaints(script: &str, named: &str) -> Vec<String> {`

Every way a `run:` step reaches the warning policy from inside its script.

The `env:` mappings this contract compares are declarations. A `run:` scalar
is not, and one line of one --
`echo "RUSTFLAGS=-A warnings" >> "$GITHUB_ENV"` -- rebinds the variable for
**every later step of the same job** while the document declares an `env:`
nowhere. Every field-set equality passes, the pinned workflow line is
untouched, and the Cargo steps that follow compile under a policy no mapping
in this file states. `MUT-RUSTFLAGS-PERSISTED-VIA-GITHUB-ENV` and its three
siblings are that line in bash, in a bash heredoc, in PowerShell and through
the `${{ github.env }}` expression.

Position is deliberately not a precondition. Refusing the write wherever it
appears is strictly stronger than refusing it only where a Cargo step
follows -- which a reorder would defeat -- and no leg of this workflow has a
benign reason to name the variable at all.

**What a token scan over an opaque script can and cannot do**, stated rather
than left to be discovered. It refuses every form that spells the name,
including forms that are not writes at all: `RUSTFLAGS=-A warnings cargo
build` scopes the flags to one command without touching the env file, and it
is refused too, because the policy is set once at workflow scope and a script
that names it is doing something this contract has no model for. It does
**not** refuse a script that assembles the name from pieces. That residual is
the same one `AGGREGATE_SCRIPT`'s pin carries: a script is not a document, and
the honest bound is the one written down.

## `fn rustflags_override_complaints(node: &Yaml, named: &str) -> Vec<String> {`

The guarded names a node's own `env:` may not bind, wherever that node sits.

Keys are matched case-insensitively and whole. A `RustFlags:` binding is a
no-op on the two Unix legs and the authoritative value on `windows-latest`,
so a case-sensitive reading refuses it on no platform and a substring reading
would refuse `RUSTFLAGS_EXTRA` on all three.

## `pub(super) fn workflow_complaints(doc: &Yaml) -> Vec<String> {`

Every audit, so a mutation is refused by the contract as a whole.

## `pub(super) fn complaint_codes(complaints: &[String]) -> BTreeSet<String> {`

The `[kebab-code]` each complaint opens with.

## `pub(super) struct WorkflowEscape {`

An escape this oracle must refuse, as a mutation of the real workflow.

Every row is a change that the substring oracle this section replaces
accepted, or that its own doc comment enumerated as still open. The first two
are `BRIDGE-CI-SHAPE-TEST-IS-A-SUBSTRING-ORACLE`'s; the `MUT-CI-*` names are
kept from `.github/scripts/test-docs-consistency.sh`, which recorded them as
history when that gate withdrew its claim over this surface -- a parsed
document is what lets the claim come back as an equality.

When `lint (macos)` moved to `ubuntu-latest` and `msrv` to one `include:`
entry per target, the rows on the two jobs were re-spelled with the same
mutations: the `lint-macos` rows anchor the cross gate's run line or its
`runs-on: ubuntu-latest`, and `MUT-GATE-RUNNER-DRIFT` now moves that job to
`windows-latest`. `MUT-GATE-STEP-RETARGETED` stands on `lint (windows)`,
because a step's `CARGO_BUILD_TARGET` no longer retargets a gate that names its
tuple with `--target`. `MUT-MSRV-MATRIX-NARROWED` deletes the macOS entry and
`MUT-MSRV-MATRIX-EXCLUDED` puts an `exclude:` naming it above `include:`. Five
rows came with the move: `MUT-GATE-CROSS-TARGET-DROPPED` (`no-gate-job`),
`MUT-GATE-TOOLCHAIN-TARGET-DROPPED` (`gate-toolchain-target`),
`MUT-MSRV-CROSS-TARGET-DROPPED` (`msrv-job-matrix`),
`MUT-MSRV-COMMAND-UNTARGETED` (`msrv-job-command`) and
`MUT-MSRV-TOOLCHAIN-TARGET-DROPPED` (`msrv-job-toolchain`). Each is described
where the check that refuses it is: under [`ci_gate_complaints`] and
[`ci_msrv_job_complaints`].

## `pub(super) struct WorkflowEscape` › `pub(super) name: &'static str,`

The name this escape is recorded under.

## `pub(super) struct WorkflowEscape` › `pub(super) escape: &'static str,`

What passes while the gate does not run.

## `pub(super) struct WorkflowEscape` › `pub(super) job: Option<&'static str>,`

The job whose block the anchor must appear in exactly once, or `None`
when the mutation is above the jobs (a workflow-level `defaults:`) or
spans a job header (a whole added job), where the anchor must be unique
in the document instead.

## `pub(super) struct WorkflowEscape` › `pub(super) refused_as: &'static str,`

The complaint code the contract must produce. A code, not a phrase: a
mutation refused for an unrelated reason is not a refusal of this escape.

## `pub(super) fn mutate_workflow(`

Replace `anchor` with `replacement` inside one job's block.

Scoped to the block, and the anchor must occur in it exactly once: a mutation
that lands somewhere unintended, or that no longer matches because the
workflow moved, is a mutation nobody measured. Both are failures here rather
than a quietly weaker test.

## `pub(super) fn shard_selections(table: &[&[&str]]) -> Vec<String> {`

The libtest arguments of each shard an ordered table of filter lists makes:
the shard rule, in one place.

A table of `n` lists makes `n + 1` shards. Shard `k` gets the filters of list
`k`, in order, then `--skip F` for each filter `F` of every list before it,
in table order; the last shard gets only the skips, one for every filter of
the table. The words of each are joined by single spaces. So
`[&["a::", "c::"], &["b::"]]` gives `a:: c::`, `b:: --skip a:: --skip c::`
and `--skip a:: --skip c:: --skip b::`, and an empty table gives one empty
string: one shard with no arguments, which is the whole suite.

That divides any set of test names, whatever the filters are, provided each
is one word libtest reads as a filter and no list is empty. libtest runs a
test when its name contains any of the filters, or no filter was given, and
contains no skip string. Take the first list in the table holding a filter
the name contains. Every shard before that list's asks only for filters the
name does not contain; that list's own shard asks for one it does contain
and skips only filters the name does not contain; and every shard after it,
the last included, skips that filter. A name that contains no filter is run
by the last shard and by no other. So each name runs in exactly one shard,
and nothing about prefixes or modules is assumed, which is as well: a filter
is a substring of the whole path.

The proviso is the caller's. An empty filter is no word at all, one with
whitespace in it is two, and one that starts with `-` is an option; an empty
list is a shard with no filter, which runs every test no earlier shard ran.
`the_shard_tables_run_every_test_in_exactly_one_shard` in `super` refuses
each in the three tables of `super::ci_model`, and a filter listed twice in
a table, whose later place skips it or names it again. The same test holds
the count of each table's strings, the first of each and the last Windows
one, and the two Linux strings to written-out literals.

One function, so that the tables and anything derived from them cannot
disagree about the rule. The Windows shards' matrix is the first thing derived
from one: [`windows_shard_legs`] takes each leg's arguments from here, and
`ci.yml` is held to them. The `test-linux` and `test-macos` matrices are the second:
[`test_legs`] takes the five macOS legs' arguments and the two Linux legs'
from here. Until the Linux suite was divided its one leg's came from the
empty list, whose one string is the whole suite.

## `pub(super) fn libtest_runs(selection: &str, name: &str) -> bool {`

Whether libtest, handed one of [`shard_selections`]' strings as its
arguments, selects the test called `name`: a model of its rule, so that a
table can be shown to divide a set of names without a harness being run.

The selection is split on whitespace. The word after each `--skip` is a skip
string and every other word is a filter. The answer is true when there is no
filter or the name contains one, and the name contains no skip string.
Containment, not equality and not a prefix: that is libtest's rule without
`--exact`. A `--skip` with no word after it selects nothing, since a string
the model cannot read as a selection must not count as one that runs a test.

It models those two kinds of word and no other. Any other option would be
read as a filter, and [`shard_selections`] emits none. It also says nothing
of `#[ignore]`: a selected test that is ignored is reported ignored by its
shard, as it is by the whole suite.
