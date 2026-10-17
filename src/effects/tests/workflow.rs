//! Extended notes: `docs/internals/effects/tests/workflow.md`

#![forbid(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use yaml_rust2::{Yaml, YamlLoader};

use super::ci_model::{
    ACTION_INPUTS, AGGREGATE_JOB, AGGREGATE_JOB_FIELDS, AGGREGATE_SCRIPT, AGGREGATE_SHELL,
    AGGREGATE_STEP_FIELDS, CI_TARGETS, CI_WORKFLOW, CLIPPY_GATE, COVERAGE_COMMAND,
    COVERAGE_DOWNLOAD_INPUTS, COVERAGE_JOB, COVERAGE_JOB_FIELDS, COVERAGE_JOB_NAME, COVERAGE_NEEDS,
    COVERAGE_RUNNER, CiTarget, DEFAULTS_FIELDS, DEFAULTS_RUN_FIELDS, DOWNLOAD_ACTION,
    ENCODED_RUSTFLAGS_KEY, GATE_JOB_FIELDS, GATE_SCRIPTS, GOLDEN_IMAGE_TOOLCHAIN, HOSTED_LANE,
    KNOWN_SHELLS, LANE_STEP_FIELDS, MACOS_GROUP, MSRV_COMMAND, MSRV_JOB, MSRV_JOB_FIELDS,
    OPTIONAL_DEFAULTS_FIELD, PARTITION_EVENTS, PARTITION_TABLE, PARTITION_TABLE_LINES,
    PINNED_ACTIONS, QUEUE_LANE, REQUIRED_CONTEXT, RUSTFLAGS_KEY, RUSTFLAGS_VALUE,
    SHARD_UPLOAD_INPUTS, STABLE_TOOLCHAIN, STEP_FIELDS, STRATEGY_FIELDS, TEST_COMMAND,
    TEST_JOB_FIELDS, TEST_NAME, TEST_SCRIPTS, TEST_SHARD_RUNNER, TEST_SHARD_SCRIPT, TEST_STEP_ENV,
    TEST_STEP_FIELDS, TEST_WHOLE_SUITE_RUNNER, TEST_WINDOWS_JOB, TEST_WINDOWS_JOB_FIELDS,
    TEST_WINDOWS_NAME, TEST_WINDOWS_PLATFORM, TEST_WINDOWS_RUNS_ON, TEST_WINDOWS_SCRIPTS,
    TEST_WINDOWS_STEP_ENV, TEST_WINDOWS_TIMEOUT, TEST_WINDOWS_TOOLCHAIN_COMPONENTS,
    TOOLCHAIN_ACTION, TOOLCHAIN_COMPONENTS, UPLOAD_ACTION, UPLOAD_ACTION_PREFIX, WHOLE_SUITE,
    WHOLE_SUITE_LANE, WINDOWS_BUILD_WITNESS, WINDOWS_COMPILER_CHECK_LINES, WINDOWS_GROUP,
    WINDOWS_SHARD_SCRIPT, WINDOWS_TEST_WITNESS, WORKFLOW_ENV, WORKFLOW_FIELDS, WORKFLOW_PERMISSIONS,
};
use super::repo_root;

pub(super) fn ci_workflow_text() -> String {
    fs::read_to_string(repo_root().join(CI_WORKFLOW))
        .expect(CI_WORKFLOW)
        .replace("\r\n", "\n")
}

pub(super) fn parse_workflow(text: &str) -> Result<Yaml, String> {
    let mut documents =
        YamlLoader::load_from_str(text).map_err(|error| format!("[parse] {error}"))?;
    if documents.len() != 1 {
        return Err(format!(
            "[parse] expected exactly one YAML document, found {}",
            documents.len()
        ));
    }
    Ok(documents.remove(0))
}

pub(super) fn field<'a>(node: &'a Yaml, key: &str) -> Option<&'a Yaml> {
    node.as_hash()?
        .iter()
        .find(|(name, _)| name.as_str() == Some(key))
        .map(|(_, value)| value)
}

pub(super) fn field_names(node: &Yaml) -> BTreeSet<String> {
    match node.as_hash() {
        Some(hash) => hash
            .keys()
            .map(|key| match key.as_str() {
                Some(name) => name.to_owned(),
                None => format!("{key:?}"),
            })
            .collect(),
        None => BTreeSet::new(),
    }
}

pub(super) fn scalar<'a>(node: &'a Yaml, key: &str) -> Option<&'a str> {
    field(node, key).and_then(Yaml::as_str)
}

pub(super) fn steps_of(job: &Yaml) -> &[Yaml] {
    field(job, "steps")
        .and_then(Yaml::as_vec)
        .map_or(&[][..], Vec::as_slice)
}

fn scalar_set(node: &Yaml) -> Option<BTreeSet<String>> {
    node.as_vec()?
        .iter()
        .map(|item| item.as_str().map(str::to_owned))
        .collect()
}

fn scalar_map(node: &Yaml) -> Option<BTreeMap<String, String>> {
    node.as_hash()?
        .iter()
        .map(|(key, value)| Some((key.as_str()?.to_owned(), value.as_str()?.to_owned())))
        .collect()
}

fn gate_stem(job: &str) -> String {
    job.to_uppercase().replace('-', "_")
}

fn unexpected(found: &BTreeSet<String>, allowed: &[&str]) -> Vec<String> {
    let allowed: BTreeSet<&str> = allowed.iter().copied().collect();
    found
        .iter()
        .filter(|name| !allowed.contains(name.as_str()))
        .cloned()
        .collect()
}

fn field_complaints(node: &Yaml, required: &[&str], optional: &[&str]) -> Vec<String> {
    let declared = field_names(node);
    let allowed: Vec<&str> = required.iter().chain(optional.iter()).copied().collect();
    let mut out: Vec<String> = unexpected(&declared, &allowed)
        .into_iter()
        .map(|name| format!("declares `{name}`, which this contract does not model"))
        .collect();
    for name in required {
        if !declared.contains(*name) {
            out.push(format!("does not declare `{name}`"));
        }
    }
    out
}

fn defaults_shell<'a>(node: &'a Yaml, where_: &str, out: &mut Vec<String>) -> Option<&'a str> {
    let defaults = field(node, "defaults")?;
    for complaint in field_complaints(defaults, &DEFAULTS_FIELDS, &[]) {
        out.push(format!(
            "[defaults-shape] {where_}'s `defaults:` {complaint}"
        ));
    }
    let run = field(defaults, "run")?;
    for complaint in field_complaints(run, &[], &DEFAULTS_RUN_FIELDS) {
        out.push(format!(
            "[defaults-shape] {where_}'s `defaults.run:` {complaint}"
        ));
    }
    scalar(run, "shell")
}

fn effective_shell(
    doc: &Yaml,
    job: &Yaml,
    step: &Yaml,
    target: &CiTarget,
    out: &mut Vec<String>,
) -> String {
    let job_default = defaults_shell(job, "the job", out);
    let workflow_default = defaults_shell(doc, "the workflow", out);
    scalar(step, "shell")
        .or(job_default)
        .or(workflow_default)
        .unwrap_or(target.default_shell)
        .to_owned()
}

fn shell_complaints(
    doc: &Yaml,
    job: &Yaml,
    step: &Yaml,
    target: &CiTarget,
    named: &str,
    expected: &str,
) -> Vec<String> {
    let mut out = Vec::new();
    if field(step, "shell").is_some() && scalar(step, "shell").is_none() {
        out.push(format!(
            "[step-shell] {named} declares a `shell:` that is not a string, so what it \
             resolves to is not something this contract can read"
        ));
    }
    let resolved = effective_shell(doc, job, step, target, &mut out);
    if !KNOWN_SHELLS.contains(&resolved.as_str()) {
        out.push(format!(
            "[step-shell] {named} resolves to shell `{resolved}`, which is not one GitHub \
             defines. A custom shell is a command template run as `<template> <script file>`, \
             so it can succeed without executing the script at all."
        ));
    } else if resolved != expected {
        out.push(format!(
            "[step-shell] {named} resolves to shell `{resolved}` on `{}`, not `{expected}`. \
             The resolution is step, then job `defaults.run.shell`, then workflow \
             `defaults.run.shell`, then the platform default.",
            target.runner
        ));
    }
    out
}

fn stem_collisions(jobs: &BTreeSet<String>) -> BTreeMap<String, Vec<String>> {
    let mut by_stem: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for job in jobs {
        by_stem.entry(gate_stem(job)).or_default().push(job.clone());
    }
    by_stem.retain(|_, named| named.len() > 1);
    by_stem
}

fn ci_gate_complaints(doc: &Yaml) -> Vec<String> {
    let mut out = Vec::new();
    for complaint in field_complaints(doc, &WORKFLOW_FIELDS, &OPTIONAL_DEFAULTS_FIELD) {
        out.push(format!(
            "[unexpected-workflow-field] the workflow {complaint}"
        ));
    }
    let Some(jobs) = field(doc, "jobs") else {
        out.push("[jobs] the workflow declares no `jobs:` mapping".to_owned());
        return out;
    };
    let job_names = field_names(jobs);
    if job_names.is_empty() {
        out.push("[jobs] `jobs:` is not a mapping with named jobs".to_owned());
        return out;
    }

    for target in &CI_TARGETS {
        let gates: Vec<&String> = job_names
            .iter()
            .filter(|name| {
                let Some(job) = field(jobs, name) else {
                    return false;
                };
                scalar(job, "runs-on") == Some(target.runner)
                    && steps_of(job)
                        .iter()
                        .any(|step| scalar(step, "run") == Some(CLIPPY_GATE))
            })
            .collect();
        if gates.len() != 1 {
            out.push(format!(
                "[no-gate-job] expected exactly one job whose `runs-on:` is `{}` and one of \
                 whose steps has `run:` equal to `{CLIPPY_GATE}`, found {gates:?}. Without it \
                 every body the `{}` tuple compiles is outside the denylist's reach on every \
                 job CI runs.",
                target.runner, target.triple
            ));
            continue;
        }
        let gate = gates[0];
        let Some(job) = field(jobs, gate) else {
            continue;
        };

        for complaint in field_complaints(job, &GATE_JOB_FIELDS, &OPTIONAL_DEFAULTS_FIELD) {
            out.push(format!(
                "[unexpected-job-field] `{gate}` {complaint}. A field this contract does not \
                 model can disable the job (`if:`) or absolve its failure \
                 (`continue-on-error:`) while every other check here still passes."
            ));
        }
        for (index, step) in steps_of(job).iter().enumerate() {
            let names = field_names(step);
            let strange = unexpected(&names, &STEP_FIELDS);
            if !strange.is_empty() {
                out.push(format!(
                    "[unexpected-step-field] `{gate}` step {index} declares {strange:?}, which \
                     this contract does not know about; `if:` and `continue-on-error:` are \
                     absent from the allowed set deliberately."
                ));
            }
            if scalar(step, "run").is_some() {
                out.extend(shell_complaints(
                    doc,
                    job,
                    step,
                    target,
                    &format!("`{gate}` step {index}"),
                    target.default_shell,
                ));
            }
        }
        out.extend(step_pin_complaints(
            job,
            gate,
            "gate-run-script",
            &GATE_SCRIPTS,
        ));
        out.extend(checkout_complaints(job, gate, "gate-checkout"));
        out.extend(toolchain_complaints(
            job,
            gate,
            "gate-toolchain",
            Some(STABLE_TOOLCHAIN),
        ));
    }

    out.extend(aggregate_complaints(doc, jobs, &job_names));
    out
}

fn aggregate_complaints(doc: &Yaml, jobs: &Yaml, job_names: &BTreeSet<String>) -> Vec<String> {
    let mut out = Vec::new();
    let Some(aggregate) = field(jobs, AGGREGATE_JOB) else {
        return vec![format!(
            "[aggregate-missing] no `{AGGREGATE_JOB}` job, so nothing publishes the \
             `{REQUIRED_CONTEXT}` context branch protection requires"
        )];
    };

    for complaint in field_complaints(aggregate, &AGGREGATE_JOB_FIELDS, &OPTIONAL_DEFAULTS_FIELD) {
        out.push(format!(
            "[unexpected-job-field] `{AGGREGATE_JOB}` {complaint}"
        ));
    }
    if scalar(aggregate, "name") != Some(REQUIRED_CONTEXT) {
        out.push(format!(
            "[aggregate-context-name] `{AGGREGATE_JOB}` publishes `{:?}`, not \
             `{REQUIRED_CONTEXT}`. Branch protection names one context; a job that \
             publishes another leaves the required one missing forever.",
            scalar(aggregate, "name")
        ));
    }
    if scalar(aggregate, "if") != Some("always()") {
        out.push(format!(
            "[aggregate-condition] `{AGGREGATE_JOB}` runs under `{:?}`, not `always()`. \
             Without `always()` a failed or cancelled dependency *skips* the aggregate, \
             and a skipped required check never settles rather than settling red.",
            scalar(aggregate, "if")
        ));
    }

    let mut expected_needs: BTreeSet<String> = job_names.clone();
    expected_needs.remove(AGGREGATE_JOB);

    let collisions = stem_collisions(&expected_needs);
    if !collisions.is_empty() {
        out.push(format!(
            "[aggregate-stem-collision] these job ids share a gate variable stem: \
             {collisions:?}. `{AGGREGATE_JOB}` reads one `<STEM>_RESULT` per gate, so one of \
             each colliding pair is unreadable -- and because the expected env mapping and \
             the expected loop list are built from the same stems, both would compare equal \
             to a workflow that checks only one of them. Nothing below is derived while this \
             holds."
        ));
        return out;
    }

    let needs = field(aggregate, "needs").and_then(scalar_set);
    if needs.as_ref() != Some(&expected_needs) {
        out.push(format!(
            "[aggregate-needs] `{AGGREGATE_JOB}` needs {needs:?}, not exactly \
             {expected_needs:?} -- the set of every other job in this workflow. A gate the \
             aggregate does not depend on can fail while branch protection settles green."
        ));
    }
    let wired = needs.unwrap_or(expected_needs);

    let expected_env: BTreeMap<String, String> = wired
        .iter()
        .map(|job| {
            (
                format!("{}_RESULT", gate_stem(job)),
                format!("${{{{ needs.{job}.result }}}}"),
            )
        })
        .collect();

    let steps = steps_of(aggregate);
    if steps.len() != 1 {
        out.push(format!(
            "[aggregate-steps] `{AGGREGATE_JOB}` has {} steps, not exactly one",
            steps.len()
        ));
        return out;
    }
    let step = &steps[0];
    let strange = unexpected(&field_names(step), &AGGREGATE_STEP_FIELDS);
    if !strange.is_empty() {
        out.push(format!(
            "[unexpected-step-field] `{AGGREGATE_JOB}`'s step declares {strange:?}"
        ));
    }

    let on_ubuntu = CI_TARGETS
        .iter()
        .find(|target| Some(target.runner) == scalar(aggregate, "runs-on"));
    match on_ubuntu {
        Some(target) => out.extend(shell_complaints(
            doc,
            aggregate,
            step,
            target,
            &format!("`{AGGREGATE_JOB}`'s step"),
            AGGREGATE_SHELL,
        )),
        None => out.push(format!(
            "[step-shell] `{AGGREGATE_JOB}` runs on {:?}, which is not a runner this contract \
             models, so the shell its script resolves to is undecidable here",
            scalar(aggregate, "runs-on")
        )),
    }

    let env = field(step, "env").and_then(scalar_map);
    if env.as_ref() != Some(&expected_env) {
        out.push(format!(
            "[aggregate-env] `{AGGREGATE_JOB}`'s step binds {env:#?}, not exactly \
             {expected_env:#?}. Each name must read the result of the job it names; a \
             binding that reads a sibling passes an existence check and reports green \
             over this platform's failure."
        ));
    }

    let script = scalar(step, "run");
    if script != Some(AGGREGATE_SCRIPT) {
        out.push(format!(
            "[aggregate-script] `{AGGREGATE_JOB}`'s script is not the pinned one. \
             Found:\n{script:?}\nPinned:\n{AGGREGATE_SCRIPT:?}"
        ));
    }

    let expected_stems: BTreeSet<String> = wired.iter().map(|job| gate_stem(job)).collect();
    let headers: Vec<&str> = script
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("for "))
        .collect();
    if headers.len() != 1 {
        out.push(format!(
            "[aggregate-loop] `{AGGREGATE_JOB}`'s script has {} `for` headers, not exactly \
             one: {headers:?}",
            headers.len()
        ));
    } else if let Some(listed) = headers[0]
        .strip_prefix("for gate in ")
        .and_then(|rest| rest.strip_suffix("; do"))
    {
        let named: BTreeSet<String> = listed.split_whitespace().map(str::to_owned).collect();
        if named != expected_stems {
            out.push(format!(
                "[aggregate-loop] the required-gate loop names {named:?}, not exactly \
                 {expected_stems:?} -- the stems of every job in `needs:`. A gate in `needs` \
                 that the loop never reads can fail without failing the aggregate."
            ));
        }
    } else {
        out.push(format!(
            "[aggregate-loop] the loop header is not `for gate in <gates>; do`: {:?}",
            headers[0]
        ));
    }

    out
}

pub(super) fn ci_test_job_complaints(doc: &Yaml) -> Vec<String> {
    let mut out = Vec::new();
    let Some(jobs) = field(doc, "jobs") else {
        return vec!["[jobs] the workflow declares no `jobs:` mapping".to_owned()];
    };
    let Some(job) = field(jobs, "test") else {
        return vec!["[test-job-missing] no `test` job, so nothing runs these fixtures".to_owned()];
    };

    for complaint in field_complaints(job, &TEST_JOB_FIELDS, &OPTIONAL_DEFAULTS_FIELD) {
        out.push(format!("[unexpected-job-field] `test` {complaint}"));
    }
    if scalar(job, "name") != Some(TEST_NAME) {
        out.push(format!(
            "[test-job-name] the `test` job is named {:?}, not `{TEST_NAME}`. Three macOS legs \
             share one runner label, so a name that does not carry the shard publishes three \
             checks under one name, and nothing reading the run can tell which shard a result \
             belongs to.",
            scalar(job, "name")
        ));
    }
    for (index, step) in steps_of(job).iter().enumerate() {
        let script = scalar(step, "run");
        let runs_the_suite = script == Some(TEST_COMMAND) || script == Some(TEST_SHARD_SCRIPT);
        let allowed: &[&str] = if runs_the_suite {
            &TEST_STEP_FIELDS
        } else if uploads_a_record(step) {
            &LANE_STEP_FIELDS
        } else {
            &STEP_FIELDS
        };
        let strange = unexpected(&field_names(step), allowed);
        if !strange.is_empty() {
            out.push(format!(
                "[unexpected-step-field] `test` step {index} declares {strange:?}"
            ));
        }
        if runs_the_suite {
            out.extend(step_env_complaints(
                step,
                &format!("`test` step {index}"),
                "test-step-env",
                &TEST_STEP_ENV,
            ));
        }
        if script.is_some() {
            for target in CI_TARGETS.iter().filter(|target| hosts_tests(target)) {
                out.extend(shell_complaints(
                    doc,
                    job,
                    step,
                    target,
                    &format!("`test` step {index} on `{}`", target.runner),
                    target.default_shell,
                ));
            }
        }
    }

    for command in [TEST_COMMAND, TEST_SHARD_SCRIPT] {
        let running = steps_of(job)
            .iter()
            .filter(|step| scalar(step, "run") == Some(command))
            .count();
        if running != 1 {
            out.push(format!(
                "[test-job-command] the `test` job has {running} steps whose `run:` is exactly \
                 `{command}`, not one. These fixtures are only evidence of anything in a job \
                 that executes them: the whole suite on the ubuntu leg, one shard of it on each \
                 macOS leg."
            ));
        }
    }
    out.extend(suite_lane_complaints(
        job,
        "test",
        "test-suite-lane",
        &[
            (TEST_COMMAND, WHOLE_SUITE_LANE),
            (TEST_SHARD_SCRIPT, HOSTED_LANE),
        ],
    ));
    out.extend(upload_complaints(
        job,
        "test",
        "test-job-upload",
        TEST_SHARD_SCRIPT,
    ));
    out.extend(checkout_complaints(job, "test", "test-job-checkout"));
    out.extend(step_pin_complaints(
        job,
        "test",
        "test-job-run-script",
        &TEST_SCRIPTS,
    ));
    out.extend(toolchain_complaints(
        job,
        "test",
        "test-job-toolchain",
        Some(STABLE_TOOLCHAIN),
    ));

    out.extend(strategy_complaints(
        job,
        "test",
        "test-job-matrix",
        "include",
    ));
    let listed = field(job, "strategy")
        .and_then(|strategy| field(strategy, "matrix"))
        .and_then(matrix_entries);
    match test_matrix() {
        Err(problem) => out.push(format!("[test-job-matrix] {problem}")),
        Ok(expected) => {
            let hosted: BTreeSet<&str> = CI_TARGETS
                .iter()
                .filter(|target| hosts_tests(target))
                .map(|target| target.runner)
                .collect();
            let modelled: BTreeSet<&str> =
                expected.iter().map(|(runner, _)| runner.as_str()).collect();
            if modelled != hosted {
                out.push(format!(
                    "[test-job-matrix] the modelled `test` matrix runs on {modelled:?}, not on \
                     exactly the platforms that host these fixtures, {hosted:?}"
                ));
            }
            if listed.as_ref() != Some(&expected) {
                out.push(format!(
                    "[test-job-matrix] the `test` job's matrix is {listed:?}, not exactly \
                     {expected:?}: the whole suite on `{TEST_WHOLE_SUITE_RUNNER}` and one leg per \
                     `{MACOS_GROUP}` shard of `{PARTITION_TABLE}` on `{TEST_SHARD_RUNNER}`, in the \
                     table's order. A leg missing is a shard nobody runs; a leg added is a shard \
                     the verifier refuses or a platform the cfg census does not model."
                ));
            }
        }
    }
    if scalar(job, "runs-on") != Some("${{ matrix.os }}") {
        out.push(format!(
            "[test-job-matrix] the `test` job's `runs-on:` is {:?}, so the matrix above \
             decides nothing",
            scalar(job, "runs-on")
        ));
    }

    let toolchains: Vec<&Yaml> = steps_of(job)
        .iter()
        .filter(|step| {
            scalar(step, "uses").is_some_and(|uses| uses.starts_with("dtolnay/rust-toolchain@"))
        })
        .collect();
    if toolchains.len() != 1 {
        out.push(format!(
            "[test-job-toolchain] the `test` job has {} `dtolnay/rust-toolchain` steps, not \
             one",
            toolchains.len()
        ));
        return out;
    }
    let with = field(toolchains[0], "with");
    let components: BTreeSet<&str> = with
        .and_then(|with| scalar(with, "components"))
        .map(|list| list.split(',').map(str::trim).collect())
        .unwrap_or_default();
    if !components.contains("clippy") {
        out.push(format!(
            "[test-job-toolchain] the `test` job installs components {components:?}, which \
             do not include `clippy`, so `every_declared_effect_denial_refuses_for_the_reason\
             _it_declares` cannot run there: `dtolnay/rust-toolchain` installs the minimal \
             profile and `clippy-driver` is not in it."
        ));
    }
    if with.and_then(|with| scalar(with, "toolchain")) != Some("stable") {
        out.push(format!(
            "[test-job-toolchain] the `test` job selects toolchain {:?}, not `stable`",
            with.and_then(|with| scalar(with, "toolchain"))
        ));
    }
    out
}

fn uploads_a_record(step: &Yaml) -> bool {
    scalar(step, "uses").is_some_and(|uses| uses.starts_with(UPLOAD_ACTION_PREFIX))
}

fn suite_lane_complaints(
    job: &Yaml,
    named: &str,
    code: &str,
    suites: &[(&str, &str)],
) -> Vec<String> {
    let mut out = Vec::new();
    for (index, step) in steps_of(job).iter().enumerate() {
        let Some(script) = scalar(step, "run") else {
            continue;
        };
        let Some((_, lane)) = suites.iter().find(|(suite, _)| *suite == script) else {
            continue;
        };
        if scalar(step, "if") != Some(*lane) {
            out.push(format!(
                "[{code}] `{named}` step {index} runs the suite under {:?}, not exactly \
                 `{lane}`. The whole-suite step and the shard step carry complementary \
                 conditions, so every leg runs exactly one of them: any other condition is a leg \
                 that reports success having run nothing, or one that runs the suite twice.",
                scalar(step, "if")
            ));
        }
    }
    out
}

fn upload_complaints(job: &Yaml, named: &str, code: &str, after: &str) -> Vec<String> {
    let mut out = Vec::new();
    let steps = steps_of(job);
    let uploads: Vec<usize> = steps
        .iter()
        .enumerate()
        .filter(|(_, step)| uploads_a_record(step))
        .map(|(index, _)| index)
        .collect();
    let shards: Vec<usize> = steps
        .iter()
        .enumerate()
        .filter(|(_, step)| scalar(step, "run") == Some(after))
        .map(|(index, _)| index)
        .collect();
    let ([upload], [shard]) = (uploads.as_slice(), shards.as_slice()) else {
        return vec![format!(
            "[{code}] `{named}` uploads at steps {uploads:?} and runs a shard at steps \
             {shards:?}, not once each, so a shard's record is not the one thing its leg uploads"
        )];
    };
    if *upload + 1 != steps.len() || upload < shard {
        out.push(format!(
            "[{code}] `{named}` uploads at step {upload} of {}, not as its last step, after the \
             shard at step {shard}. An upload above the suite records nothing it ran, and one \
             with steps after it succeeds for a leg that then fails.",
            steps.len()
        ));
    }
    let Some(step) = steps.get(*upload) else {
        return out;
    };
    if scalar(step, "if") != Some(HOSTED_LANE) {
        out.push(format!(
            "[{code}] `{named}` step {upload} uploads under {:?}, not exactly `{HOSTED_LANE}`. \
             Under `always()` or `failure()` a failed shard uploads a record of a run that did \
             not finish; without the shard condition the whole-suite leg fails for want of one.",
            scalar(step, "if")
        ));
    }
    if scalar(step, "uses") != Some(UPLOAD_ACTION) {
        out.push(format!(
            "[{code}] `{named}` step {upload} uses {:?}, not `{UPLOAD_ACTION}`",
            scalar(step, "uses")
        ));
    }
    let expected: BTreeMap<String, String> = SHARD_UPLOAD_INPUTS
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect();
    let with = field(step, "with").and_then(scalar_map);
    if with.as_ref() != Some(&expected) {
        out.push(format!(
            "[{code}] `{named}` step {upload} uploads with {with:#?}, not exactly {expected:#?}. \
             The name carries the shard and the run attempt, so a re-run never collides with the \
             record it supersedes and the verifier can tell the two apart; the path is the \
             directory the runner wrote; and an upload that finds nothing fails the leg."
        ));
    }
    out
}

fn strategy_complaints(job: &Yaml, named: &str, code: &str, matrix_field: &str) -> Vec<String> {
    let mut out = Vec::new();
    let Some(strategy) = field(job, "strategy") else {
        return vec![format!("[{code}] `{named}` declares no `strategy:`, so it runs one leg")];
    };
    for complaint in field_complaints(strategy, &STRATEGY_FIELDS, &[]) {
        out.push(format!(
            "[{code}] `{named}`'s `strategy:` {complaint}. `max-parallel:` is a limit this \
             contract does not model."
        ));
    }
    if field(strategy, "fail-fast").and_then(Yaml::as_bool) != Some(false) {
        out.push(format!(
            "[{code}] `{named}`'s `fail-fast:` is not `false`, so one leg's failure cancels the \
             others before they report"
        ));
    }
    if let Some(matrix) = field(strategy, "matrix") {
        for complaint in field_complaints(matrix, &[matrix_field], &[]) {
            out.push(format!(
                "[{code}] `{named}`'s `matrix:` {complaint}. `exclude:` removes a leg the list \
                 still names, and a second list multiplies the legs."
            ));
        }
    }
    out
}

fn matrix_entries(matrix: &Yaml) -> Option<Vec<(String, String)>> {
    let keys = BTreeSet::from(["os".to_owned(), "shard".to_owned()]);
    field(matrix, "include")?
        .as_vec()?
        .iter()
        .map(|entry| {
            if field_names(entry) != keys {
                return None;
            }
            Some((
                scalar(entry, "os")?.to_owned(),
                scalar(entry, "shard")?.to_owned(),
            ))
        })
        .collect()
}

struct Partition {
    shards: Vec<(String, String)>,
    lanes: BTreeMap<String, BTreeSet<String>>,
}

impl Partition {
    fn shards(&self, group: &str) -> Vec<String> {
        self.shards
            .iter()
            .filter(|(owner, _)| owner == group)
            .map(|(_, shard)| shard.to_owned())
            .collect()
    }

    fn partitions(&self, event: &str, group: &str) -> bool {
        self.lanes
            .get(event)
            .is_some_and(|groups| groups.contains(group))
    }
}

fn partition() -> Result<Partition, String> {
    let text = fs::read_to_string(repo_root().join(PARTITION_TABLE))
        .map_err(|error| format!("{PARTITION_TABLE} cannot be read: {error}"))?
        .replace("\r\n", "\n");
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();
    if lines != PARTITION_TABLE_LINES {
        return Err(format!(
            "{PARTITION_TABLE} holds the lines {lines:#?}, not exactly the pinned \
             {PARTITION_TABLE_LINES:#?}. The table decides which tests each partitioned leg runs \
             and which legs the verifier expects; a change to it is a change to this contract, \
             made in the same commit."
        ));
    }
    let mut partition = Partition {
        shards: Vec::new(),
        lanes: BTreeMap::new(),
    };
    for line in PARTITION_TABLE_LINES {
        let words: Vec<&str> = line.split_whitespace().collect();
        match words.as_slice() {
            ["shard", group, shard, ..] => {
                partition.shards.push(((*group).to_owned(), (*shard).to_owned()));
            }
            ["lane", event, groups @ ..] => {
                partition.lanes.insert(
                    (*event).to_owned(),
                    groups.iter().map(|group| (*group).to_owned()).collect(),
                );
            }
            _ => {}
        }
    }
    for event in PARTITION_EVENTS {
        if !partition.lanes.contains_key(event) {
            return Err(format!("{PARTITION_TABLE} has no lane for `{event}`"));
        }
    }
    Ok(partition)
}

fn test_matrix() -> Result<Vec<(String, String)>, String> {
    let partition = partition()?;
    for event in PARTITION_EVENTS {
        if !partition.partitions(event, MACOS_GROUP) {
            return Err(format!(
                "{PARTITION_TABLE} does not partition `{MACOS_GROUP}` on `{event}`, and the \
                 `test` job's matrix is the same on every event"
            ));
        }
    }
    let mut legs = vec![(TEST_WHOLE_SUITE_RUNNER.to_owned(), WHOLE_SUITE.to_owned())];
    legs.extend(
        partition
            .shards(MACOS_GROUP)
            .into_iter()
            .map(|shard| (TEST_SHARD_RUNNER.to_owned(), shard)),
    );
    Ok(legs)
}

fn json_list(items: &[String]) -> String {
    let quoted: Vec<String> = items.iter().map(|item| format!("\"{item}\"")).collect();
    format!("[{}]", quoted.join(", "))
}

fn test_windows_shards() -> Result<String, String> {
    let partition = partition()?;
    let pull_request = partition.partitions("pull_request", WINDOWS_GROUP);
    if pull_request != partition.partitions("push", WINDOWS_GROUP) {
        return Err(format!(
            "{PARTITION_TABLE} partitions `{WINDOWS_GROUP}` on one of `pull_request` and `push` \
             and not the other, which `{QUEUE_LANE}` cannot tell apart"
        ));
    }
    let queue = if partition.partitions("merge_group", WINDOWS_GROUP) {
        partition.shards(WINDOWS_GROUP)
    } else {
        vec![WHOLE_SUITE.to_owned()]
    };
    let mut other = vec![WHOLE_SUITE.to_owned()];
    if pull_request {
        other.extend(partition.shards(WINDOWS_GROUP));
    }
    Ok(format!(
        "${{{{ fromJSON({QUEUE_LANE} && '{}' || '{}') }}}}",
        json_list(&queue),
        json_list(&other)
    ))
}

fn hosts_tests(target: &CiTarget) -> bool {
    target.runner != TEST_WINDOWS_PLATFORM
}

fn installs_a_toolchain(step: &Yaml) -> bool {
    scalar(step, "uses").is_some_and(|uses| uses.starts_with(TOOLCHAIN_ACTION))
}

pub(super) fn ci_test_windows_job_complaints(doc: &Yaml) -> Vec<String> {
    let mut out = Vec::new();
    let Some(jobs) = field(doc, "jobs") else {
        return vec!["[jobs] the workflow declares no `jobs:` mapping".to_owned()];
    };
    let Some(job) = field(jobs, TEST_WINDOWS_JOB) else {
        return vec![format!(
            "[test-windows-missing] no `{TEST_WINDOWS_JOB}` job, so nothing runs the Windows \
             suite"
        )];
    };
    let Some(platform) = CI_TARGETS
        .iter()
        .find(|target| target.runner == TEST_WINDOWS_PLATFORM)
    else {
        return vec![format!(
            "[test-windows-platform] `{TEST_WINDOWS_PLATFORM}` is not a runner this \
             contract models, so the shell its steps resolve to is undecidable here"
        )];
    };

    for complaint in field_complaints(job, &TEST_WINDOWS_JOB_FIELDS, &OPTIONAL_DEFAULTS_FIELD) {
        out.push(format!(
            "[unexpected-job-field] `{TEST_WINDOWS_JOB}` {complaint}"
        ));
    }
    for (index, step) in steps_of(job).iter().enumerate() {
        let script = scalar(step, "run");
        let runs_the_suite =
            script == Some(WINDOWS_TEST_WITNESS) || script == Some(WINDOWS_SHARD_SCRIPT);
        let allowed: &[&str] = if runs_the_suite {
            &TEST_STEP_FIELDS
        } else if installs_a_toolchain(step) || uploads_a_record(step) {
            &LANE_STEP_FIELDS
        } else {
            &STEP_FIELDS
        };
        let strange = unexpected(&field_names(step), allowed);
        if !strange.is_empty() {
            out.push(format!(
                "[unexpected-step-field] `{TEST_WINDOWS_JOB}` step {index} declares {strange:?}. \
                 `if:` is admitted on the toolchain install, the two steps that run the suite \
                 and the upload, where each is pinned to the condition naming the legs it is \
                 written for; on any other `run:` step it is a script one leg executes and \
                 another never shows a reviewer."
            ));
        }
        if runs_the_suite {
            out.extend(step_env_complaints(
                step,
                &format!("`{TEST_WINDOWS_JOB}` step {index}"),
                "test-windows-step-env",
                &TEST_WINDOWS_STEP_ENV,
            ));
        }
        if script.is_some() {
            out.extend(shell_complaints(
                doc,
                job,
                step,
                platform,
                &format!("`{TEST_WINDOWS_JOB}` step {index}"),
                platform.default_shell,
            ));
        }
    }

    out.extend(toolchain_complaints(
        job,
        TEST_WINDOWS_JOB,
        "test-windows-toolchain",
        None,
    ));
    for (index, step) in steps_of(job).iter().enumerate() {
        if !installs_a_toolchain(step) {
            continue;
        }
        let selected = field(step, "with").and_then(|with| scalar(with, "toolchain"));
        if selected != Some(GOLDEN_IMAGE_TOOLCHAIN) {
            out.push(format!(
                "[test-windows-toolchain] `{TEST_WINDOWS_JOB}` step {index} installs toolchain \
                 {selected:?}, not `{GOLDEN_IMAGE_TOOLCHAIN}`, the golden image's. Current \
                 stable is ahead of the image, so `stable` here runs the suite on a compiler \
                 the pull-request lane never ran it on, and a test whose outcome depends on \
                 the compiler passes on one lane and fails on the other; an older version \
                 omits on the queue whatever the image's compiler enables. The pin keeps the \
                 two test lanes one compiler. It shields the queue from nothing else: \
                 `lint (windows)` compiles current stable on both events and is required by \
                 the aggregate, so a stable that moves between a pull request's green and \
                 its enqueue ejects the entry through that leg, with or without this pin."
            ));
        }
        if scalar(step, "if") != Some(HOSTED_LANE) {
            out.push(format!(
                "[test-windows-toolchain] `{TEST_WINDOWS_JOB}` step {index} installs its \
                 toolchain under {:?}, not exactly `{HOSTED_LANE}`. The install is written for \
                 the hosted legs, whose runner carries no curated compiler; the guest's image \
                 carries the compiler this job runs, and re-curation is how it moves. \
                 Unconditional, the step selects the guest's compiler from the workflow \
                 instead; inverted, the hosted legs run on whatever GitHub's image \
                 preinstalled and the guest's leg installs over its own.",
                scalar(step, "if")
            ));
        }
        let components = field(step, "with").and_then(|with| scalar(with, "components"));
        if components != Some(TEST_WINDOWS_TOOLCHAIN_COMPONENTS) {
            out.push(format!(
                "[test-windows-toolchain] `{TEST_WINDOWS_JOB}` step {index} installs components \
                 {components:?}, not exactly `{TEST_WINDOWS_TOOLCHAIN_COMPONENTS}`. \
                 `dtolnay/rust-toolchain` installs the minimal profile, and \
                 `every_declared_effect_denial_refuses_for_the_reason_it_declares` drives \
                 `clippy-driver` on the hosted lane as it does in `test`."
            ));
        }
    }

    if scalar(job, "runs-on") != Some(TEST_WINDOWS_RUNS_ON) {
        out.push(format!(
            "[test-windows-runner] `{TEST_WINDOWS_JOB}` runs on {:?}, not exactly \
             `{TEST_WINDOWS_RUNS_ON}`. The expression names both machines, by leg: \
             `{TEST_WINDOWS_PLATFORM}` for a shard, which only the merge-queue lane runs and \
             nothing waits on interactively, and the pinned label set for the whole-suite leg \
             every other build runs, whose third label names the curated image. A bare scalar \
             sends the pull-request lane to the hosted runner and its six minutes become \
             twenty-five; a bare label set sends every leg to the pull-request guest, where a \
             shard's install, conditioned on the shard and not the machine, runs over the \
             image's compiler; a subset of the labels admits any self-hosted Windows machine \
             the account registers; and a condition spelled other than `{HOSTED_LANE}` is one \
             this contract cannot evaluate -- it may route every leg as the install step \
             expects or send one to a machine the step was not written for, and an equality \
             over the text cannot tell which.",
            field(job, "runs-on")
        ));
    }
    if scalar(job, "name") != Some(TEST_WINDOWS_NAME) {
        out.push(format!(
            "[test-windows-name] `{TEST_WINDOWS_JOB}` is named {:?}, not exactly \
             `{TEST_WINDOWS_NAME}`. The guest's leg keeps the name `test (winguest)` that the \
             scheduling guards read, and each hosted shard carries its own, so no two legs \
             publish one name.",
            scalar(job, "name")
        ));
    }
    if scalar(job, "timeout-minutes") != Some(TEST_WINDOWS_TIMEOUT) {
        out.push(format!(
            "[test-windows-timeout] `{TEST_WINDOWS_JOB}`'s ceiling is {:?}, not exactly \
             `{TEST_WINDOWS_TIMEOUT}`: each hosted shard keeps the hosted lane's ceiling and the \
             guest's leg its own.",
            field(job, "timeout-minutes")
        ));
    }
    out.extend(strategy_complaints(
        job,
        TEST_WINDOWS_JOB,
        "test-windows-matrix",
        "shard",
    ));
    let shards = field(job, "strategy")
        .and_then(|strategy| field(strategy, "matrix"))
        .and_then(|matrix| scalar(matrix, "shard"));
    match test_windows_shards() {
        Err(problem) => out.push(format!("[test-windows-matrix] {problem}")),
        Ok(expected) if shards != Some(expected.as_str()) => out.push(format!(
            "[test-windows-matrix] `{TEST_WINDOWS_JOB}`'s legs are {shards:?}, not exactly \
             `{expected}`: the `{WINDOWS_GROUP}` shards of `{PARTITION_TABLE}` on the lanes the \
             table partitions it on, and the guest's whole suite on the others. A shard dropped \
             is tests nobody runs, a shard added is one the verifier refuses, and the whole \
             suite gone from the pull-request lane is the guest's witness gone with it."
        )),
        Ok(_) => {}
    }

    for (command, what) in [
        (WINDOWS_TEST_WITNESS, "the pinned suite-and-witness script"),
        (WINDOWS_SHARD_SCRIPT, "the pinned shard script"),
    ] {
        let running = steps_of(job)
            .iter()
            .filter(|step| scalar(step, "run") == Some(command))
            .count();
        if running != 1 {
            out.push(format!(
                "[test-windows-command] `{TEST_WINDOWS_JOB}` has {running} steps whose `run:` is \
                 exactly {what}, not one. Its Cargo line is `{TEST_COMMAND}`, whole or through \
                 the partition runner; the lines before it refuse to run the suite on any \
                 compiler but the image's `{GOLDEN_IMAGE_TOOLCHAIN}`, and what follows is the \
                 count, or the record, that says the suite executed rather than compiling and \
                 being handed to something that exits zero."
            ));
        }
    }
    let witness_check: Vec<&str> = WINDOWS_TEST_WITNESS
        .lines()
        .take(WINDOWS_COMPILER_CHECK_LINES)
        .collect();
    let shard_check: Vec<&str> = WINDOWS_SHARD_SCRIPT
        .lines()
        .take(WINDOWS_COMPILER_CHECK_LINES)
        .collect();
    if witness_check != shard_check || witness_check.len() != WINDOWS_COMPILER_CHECK_LINES {
        out.push(format!(
            "[test-windows-command] the shard script's first {WINDOWS_COMPILER_CHECK_LINES} \
             lines are {shard_check:?}, not the witness's compiler check {witness_check:?}: \
             every Windows leg refuses every compiler but the image's"
        ));
    }
    out.extend(suite_lane_complaints(
        job,
        TEST_WINDOWS_JOB,
        "test-windows-suite-lane",
        &[
            (WINDOWS_TEST_WITNESS, WHOLE_SUITE_LANE),
            (WINDOWS_SHARD_SCRIPT, HOSTED_LANE),
        ],
    ));
    out.extend(upload_complaints(
        job,
        TEST_WINDOWS_JOB,
        "test-windows-upload",
        WINDOWS_SHARD_SCRIPT,
    ));
    out.extend(checkout_complaints(
        job,
        TEST_WINDOWS_JOB,
        "test-windows-checkout",
    ));
    out.extend(step_pin_complaints(
        job,
        TEST_WINDOWS_JOB,
        "test-windows-run-script",
        &TEST_WINDOWS_SCRIPTS,
    ));
    out
}

pub(super) fn ci_test_coverage_job_complaints(doc: &Yaml) -> Vec<String> {
    let mut out = Vec::new();
    let Some(jobs) = field(doc, "jobs") else {
        return vec!["[jobs] the workflow declares no `jobs:` mapping".to_owned()];
    };
    let Some(job) = field(jobs, COVERAGE_JOB) else {
        return vec![format!(
            "[test-coverage-missing] no `{COVERAGE_JOB}` job, so nothing checks that the \
             partitioned legs together ran every test their harnesses listed: each shard's \
             green says only that its own selection passed"
        )];
    };
    let Some(ubuntu) = CI_TARGETS
        .iter()
        .find(|target| target.runner == COVERAGE_RUNNER)
    else {
        return vec![format!(
            "[test-coverage-runner] `{COVERAGE_RUNNER}` is not a runner this contract models"
        )];
    };

    for complaint in field_complaints(job, &COVERAGE_JOB_FIELDS, &OPTIONAL_DEFAULTS_FIELD) {
        out.push(format!(
            "[unexpected-job-field] `{COVERAGE_JOB}` {complaint}. `if:` would run it after a \
             failed or cancelled leg, over records that leg never finished, and \
             `continue-on-error:` reports success over a refusal."
        ));
    }
    if scalar(job, "name") != Some(COVERAGE_JOB_NAME) {
        out.push(format!(
            "[test-coverage-name] `{COVERAGE_JOB}` is named {:?}, not `{COVERAGE_JOB_NAME}`",
            scalar(job, "name")
        ));
    }
    let needs = field(job, "needs").and_then(scalar_set);
    let expected: BTreeSet<String> = COVERAGE_NEEDS
        .iter()
        .map(|need| (*need).to_owned())
        .collect();
    if needs.as_ref() != Some(&expected) {
        out.push(format!(
            "[test-coverage-needs] `{COVERAGE_JOB}` needs {needs:?}, not exactly {expected:?}: \
             every job that runs a partitioned leg. A leg it does not wait for can upload after \
             the verifier has read the records, or never, and without `needs` on a failed leg \
             the verifier is not skipped."
        ));
    }
    if scalar(job, "runs-on") != Some(COVERAGE_RUNNER) {
        out.push(format!(
            "[test-coverage-runner] `{COVERAGE_JOB}` runs on {:?}, not `{COVERAGE_RUNNER}`",
            field(job, "runs-on")
        ));
    }
    let steps = steps_of(job);
    for (index, step) in steps.iter().enumerate() {
        let strange = unexpected(&field_names(step), &STEP_FIELDS);
        if !strange.is_empty() {
            out.push(format!(
                "[unexpected-step-field] `{COVERAGE_JOB}` step {index} declares {strange:?}"
            ));
        }
        if scalar(step, "run").is_some() {
            out.extend(shell_complaints(
                doc,
                job,
                step,
                ubuntu,
                &format!("`{COVERAGE_JOB}` step {index}"),
                ubuntu.default_shell,
            ));
        }
    }
    out.extend(checkout_complaints(
        job,
        COVERAGE_JOB,
        "test-coverage-checkout",
    ));
    out.extend(step_pin_complaints(
        job,
        COVERAGE_JOB,
        "test-coverage-command",
        &[COVERAGE_COMMAND],
    ));
    let downloads: Vec<usize> = steps
        .iter()
        .enumerate()
        .filter(|(_, step)| scalar(step, "uses") == Some(DOWNLOAD_ACTION))
        .map(|(index, _)| index)
        .collect();
    let verifies: Vec<usize> = steps
        .iter()
        .enumerate()
        .filter(|(_, step)| scalar(step, "run") == Some(COVERAGE_COMMAND))
        .map(|(index, _)| index)
        .collect();
    if steps.len() != 3 || downloads != [1] || verifies != [2] {
        out.push(format!(
            "[test-coverage-steps] `{COVERAGE_JOB}` has {} steps, downloads with \
             `{DOWNLOAD_ACTION}` at {downloads:?} and runs `{COVERAGE_COMMAND}` at {verifies:?}, \
             not exactly: check out, download, verify. A step between them can rewrite the \
             records or the script the verifier then reads, and a verifier that never runs \
             refuses nothing.",
            steps.len()
        ));
    }
    let inputs: BTreeMap<String, String> = COVERAGE_DOWNLOAD_INPUTS
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect();
    for index in downloads {
        let with = steps
            .get(index)
            .and_then(|step| field(step, "with"))
            .and_then(scalar_map);
        if with.as_ref() != Some(&inputs) {
            out.push(format!(
                "[test-coverage-download] `{COVERAGE_JOB}` step {index} downloads with {with:#?}, \
                 not exactly {inputs:#?}. With no `run-id` or token the action reads this run's \
                 artifacts and no other's; the pattern is the shard records' prefix, and the path \
                 is the directory the verifier reads."
            ));
        }
    }
    out
}

fn checkout_complaints(job: &Yaml, named: &str, code: &str) -> Vec<String> {
    let mut out = Vec::new();
    let checkouts: Vec<usize> = steps_of(job)
        .iter()
        .enumerate()
        .filter(|(_, step)| {
            scalar(step, "uses").is_some_and(|uses| uses.starts_with("actions/checkout@"))
        })
        .map(|(index, _)| index)
        .collect();
    if checkouts != [0] {
        out.push(format!(
            "[{code}] `{named}` checks out at steps {checkouts:?}, not exactly once at step 0. \
             Every step of a job runs against the tree the checkout put there: a step above \
             the checkout runs against an empty workspace or one another action prepared, \
             and a second checkout later can replace the candidate with another tree while \
             the first, input-free one still matches."
        ));
    }
    for (index, step) in steps_of(job).iter().enumerate() {
        let checks_out =
            scalar(step, "uses").is_some_and(|uses| uses.starts_with("actions/checkout@"));
        if !checks_out {
            continue;
        }
        let Some(inputs) = field(step, "with") else {
            continue;
        };
        out.push(format!(
            "[{code}] `{named}` step {index} checks out with inputs {:?}. With no inputs the \
             action checks out the head under test; any input can point this leg at another \
             tree while every other leg reads the candidate.",
            field_names(inputs)
        ));
    }
    out
}

fn step_env_complaints(
    step: &Yaml,
    named: &str,
    code: &str,
    pinned: &[(&str, &str)],
) -> Vec<String> {
    let expected: BTreeMap<String, String> = pinned
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect();
    let env = field(step, "env").and_then(scalar_map);
    if env.as_ref() == Some(&expected) {
        return Vec::new();
    }
    vec![format!(
        "[{code}] {named} binds {env:#?}, not exactly {expected:#?}. The step that runs the \
         suite carries one declaration -- that the temporary directory the suite runs under \
         folds case, which \
         `the_temporary_object_scan_resolves_case_aliases_as_the_filesystem_does` requires \
         its native branch under and observes without -- and the map is pinned whole: a \
         dropped binding makes the leg observe the casefold branch instead of requiring it, so \
         a temporary directory that stopped folding case would pass unreported; a value \
         other than `1` on a folding leg does the same, and any other key is a step-level \
         environment that can retarget the compile."
    )]
}

fn step_pin_complaints(job: &Yaml, named: &str, code: &str, scripts: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for (index, step) in steps_of(job).iter().enumerate() {
        let unpinned_script = scalar(step, "run").filter(|script| !scripts.contains(script));
        if let Some(script) = unpinned_script {
            out.push(format!(
                "[{code}] `{named}` step {index} runs a script this contract does not pin: \
                 {script:?}. An unpinned step can move the checkout before the pinned command \
                 runs, so the pinned command runs against another tree."
            ));
        }
        let Some(uses) = scalar(step, "uses") else {
            continue;
        };
        if !PINNED_ACTIONS.contains(&uses) {
            out.push(format!(
                "[unpinned-action] `{named}` step {index} uses {uses:?}, which is not one of the \
                 pinned actions {PINNED_ACTIONS:?}; an unreviewed action, or a reviewed one at a \
                 floating tag, can check out any tree it likes."
            ));
            continue;
        }
        let Some((_, allowed)) = ACTION_INPUTS
            .iter()
            .find(|(prefix, _)| uses.starts_with(prefix))
        else {
            out.push(format!(
                "[unpinned-action-input] `{named}` step {index} uses {uses:?}, whose inputs this \
                 contract does not model at all"
            ));
            continue;
        };
        let Some(inputs) = field(step, "with") else {
            continue;
        };
        let strange = unexpected(&field_names(inputs), allowed);
        if !strange.is_empty() {
            out.push(format!(
                "[unpinned-action-input] `{named}` step {index} gives {uses:?} the inputs \
                 {strange:?}, which are not among {allowed:?}. An input can tell an allowlisted \
                 action at a pinned commit to run commands of the candidate's choosing -- \
                 `rust-cache`'s `cmd-format` wraps every command the step runs."
            ));
        }
        if uses.starts_with(TOOLCHAIN_ACTION) {
            if let Some(components) = scalar(inputs, "components") {
                if !TOOLCHAIN_COMPONENTS.contains(&components) {
                    out.push(format!(
                        "[unpinned-action-input] `{named}` step {index} asks for components \
                         {components:?}, which is not one of {TOOLCHAIN_COMPONENTS:?}. This \
                         action interpolates the value into a shell command, so an unpinned one \
                         runs whatever it contains."
                    ));
                }
            }
        }
    }
    out
}

fn toolchain_complaints(
    job: &Yaml,
    named: &str,
    code: &str,
    expected: Option<&str>,
) -> Vec<String> {
    let mut out = Vec::new();
    let steps = steps_of(job);
    let installs: Vec<usize> = steps
        .iter()
        .enumerate()
        .filter(|(_, step)| {
            scalar(step, "uses").is_some_and(|uses| uses.starts_with(TOOLCHAIN_ACTION))
        })
        .map(|(index, _)| index)
        .collect();
    let [install] = installs.as_slice() else {
        out.push(format!(
            "[{code}] `{named}` has {} steps using `{TOOLCHAIN_ACTION}`, not one, so which \
             compiler its commands run is decided by something other than this workflow",
            installs.len()
        ));
        return out;
    };
    if *install != 1 {
        out.push(format!(
            "[{code}] `{named}` installs its toolchain at step {install}, not step 1. The \
             checkout is step 0 and the compiler is chosen next: any step above the install \
             runs whatever the runner image preinstalled."
        ));
    }
    if let Some(want) = expected {
        let selected = field(&steps[*install], "with").and_then(|with| scalar(with, "toolchain"));
        if selected != Some(want) {
            out.push(format!(
                "[{code}] `{named}` step {install} installs toolchain {selected:?}, not \
                 `{want}`. The action is pinned by commit; its input is what decides which \
                 compiler runs, and a downgrade here leaves current stable compiled by no leg."
            ));
        }
    }
    out
}

fn workflow_env_complaints(doc: &Yaml) -> Vec<String> {
    let expected: BTreeMap<String, String> = WORKFLOW_ENV
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect();
    let found = field(doc, "env").and_then(scalar_map);
    if found.as_ref() == Some(&expected) {
        return Vec::new();
    }
    vec![format!(
        "[workflow-env] the workflow's `env:` is {found:#?}, not exactly {expected:#?}. The \
         whole mapping is pinned: a name this contract does not model can decide whether the \
         compiled thing runs at all -- a Cargo target runner builds every harness and executes \
         none, on one target, with every command, shell and label still matching."
    )]
}

fn workflow_permissions_complaints(doc: &Yaml) -> Vec<String> {
    let expected: BTreeMap<String, String> = WORKFLOW_PERMISSIONS
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect();
    let found = field(doc, "permissions").and_then(scalar_map);
    if found.as_ref() == Some(&expected) {
        return Vec::new();
    }
    vec![format!(
        "[workflow-permissions] the workflow's `permissions:` is {found:#?}, not exactly \
         {expected:#?}. `write-all`, or a scalar this contract cannot read as the pinned \
         mapping, hands every step a token a candidate's build scripts and tests can push \
         with -- and the guest's destruction does not recall a push."
    )]
}

pub(super) fn ci_windows_build_witness_complaints(doc: &Yaml) -> Vec<String> {
    let mut out = Vec::new();
    let Some(jobs) = field(doc, "jobs") else {
        return vec!["[jobs] the workflow declares no `jobs:` mapping".to_owned()];
    };
    let Some(platform) = CI_TARGETS
        .iter()
        .find(|target| target.runner == TEST_WINDOWS_PLATFORM)
    else {
        return vec![format!(
            "[windows-build-witness] `{TEST_WINDOWS_PLATFORM}` is not a runner this \
             contract models, so the shell its steps resolve to is undecidable here"
        )];
    };
    let carriers: Vec<String> = field_names(jobs)
        .into_iter()
        .filter(|name| {
            field(jobs, name).is_some_and(|job| {
                scalar(job, "runs-on") == Some(TEST_WINDOWS_PLATFORM)
                    && steps_of(job)
                        .iter()
                        .any(|step| scalar(step, "run") == Some(WINDOWS_BUILD_WITNESS))
            })
        })
        .collect();
    let [carrier] = carriers.as_slice() else {
        out.push(format!(
            "[windows-build-witness] expected exactly one job whose `runs-on:` is \
             `{TEST_WINDOWS_PLATFORM}` and one of whose steps has `run:` equal to \
             `{WINDOWS_BUILD_WITNESS}`, found {carriers:?}. Without it no hosted leg \
             code-generates or links the Windows tree on current stable: `cargo check` and \
             Clippy stop before codegen, and the self-hosted leg builds with the image's \
             toolchain."
        ));
        return out;
    };
    let Some(job) = field(jobs, carrier) else {
        return out;
    };
    let witnesses = steps_of(job)
        .iter()
        .filter(|step| scalar(step, "run") == Some(WINDOWS_BUILD_WITNESS))
        .count();
    if witnesses != 1 {
        out.push(format!(
            "[windows-build-witness] `{carrier}` has {witnesses} steps whose `run:` is \
             exactly `{WINDOWS_BUILD_WITNESS}`, not one"
        ));
    }
    if !steps_of(job)
        .iter()
        .any(|step| scalar(step, "run") == Some(CLIPPY_GATE))
    {
        out.push(format!(
            "[windows-build-witness] `{carrier}` carries the witness but not the Windows Clippy \
             gate `{CLIPPY_GATE}`; the witness rides the gate job so the gate contract's pins \
             on fields, shells, scripts and the checkout cover it"
        ));
    }
    for (index, step) in steps_of(job).iter().enumerate() {
        if scalar(step, "run") == Some(WINDOWS_BUILD_WITNESS) {
            out.extend(shell_complaints(
                doc,
                job,
                step,
                platform,
                &format!("`{carrier}` step {index}"),
                platform.default_shell,
            ));
        }
    }
    out
}

pub(super) fn declared_rust_version() -> String {
    let text = fs::read_to_string(repo_root().join("Cargo.toml")).expect("Cargo.toml");
    let manifest: toml::Value = toml::from_str(&text).expect("Cargo.toml parses");
    manifest
        .get("package")
        .and_then(|package| package.get("rust-version"))
        .and_then(toml::Value::as_str)
        .expect("Cargo.toml declares a `[package] rust-version` to pin the msrv leg against")
        .to_owned()
}

pub(super) fn declared_msrv_toolchain() -> String {
    three_component(&declared_rust_version())
}

pub(super) fn three_component(version: &str) -> String {
    if version.split('.').count() == 2 {
        format!("{version}.0")
    } else {
        version.to_owned()
    }
}

pub(super) fn ci_msrv_job_complaints(doc: &Yaml) -> Vec<String> {
    let mut out = Vec::new();
    let Some(jobs) = field(doc, "jobs") else {
        return vec!["[jobs] the workflow declares no `jobs:` mapping".to_owned()];
    };
    let Some(job) = field(jobs, MSRV_JOB) else {
        return vec![format!(
            "[msrv-job-missing] no `{MSRV_JOB}` job, so nothing compiles this crate on the \
             floor `Cargo.toml`'s `rust-version` publishes"
        )];
    };

    for complaint in field_complaints(job, &MSRV_JOB_FIELDS, &OPTIONAL_DEFAULTS_FIELD) {
        out.push(format!(
            "[msrv-job-field] `{MSRV_JOB}` {complaint}. `continue-on-error:` reports success \
             over a failed check -- which the aggregate then reads as success -- and `if:` \
             stops the leg running at all."
        ));
    }
    for (index, step) in steps_of(job).iter().enumerate() {
        let strange = unexpected(&field_names(step), &STEP_FIELDS);
        if !strange.is_empty() {
            out.push(format!(
                "[msrv-job-step-field] `{MSRV_JOB}` step {index} declares {strange:?}, which \
                 this contract does not model"
            ));
        }
        if scalar(step, "run").is_some() {
            for target in &CI_TARGETS {
                out.extend(shell_complaints(
                    doc,
                    job,
                    step,
                    target,
                    &format!("`{MSRV_JOB}` step {index} on `{}`", target.runner),
                    target.default_shell,
                ));
            }
        }
    }

    let running = steps_of(job)
        .iter()
        .filter(|step| scalar(step, "run") == Some(MSRV_COMMAND))
        .count();
    if running != 1 {
        out.push(format!(
            "[msrv-job-command] the `{MSRV_JOB}` job has {running} steps whose `run:` is \
             exactly `{MSRV_COMMAND}`, not one. Dropping `--locked` lets Cargo resolve past \
             the exact pins this manifest carries for the floor, and an `echo` satisfies \
             every substring reading of the same line."
        ));
    }
    out.extend(step_pin_complaints(
        job,
        MSRV_JOB,
        "msrv-run-script",
        &[MSRV_COMMAND],
    ));
    out.extend(checkout_complaints(job, MSRV_JOB, "msrv-checkout"));
    out.extend(toolchain_complaints(job, MSRV_JOB, "msrv-toolchain", None));

    let expected_runners: BTreeSet<String> = CI_TARGETS
        .iter()
        .map(|target| target.runner.to_owned())
        .collect();
    match field(job, "strategy") {
        None => out.push(format!(
            "[msrv-job-matrix] the `{MSRV_JOB}` job declares no `strategy:`, so it checks the \
             floor on one platform"
        )),
        Some(strategy) => {
            for complaint in field_complaints(strategy, &["fail-fast", "matrix"], &[]) {
                out.push(format!(
                    "[msrv-job-matrix] the `{MSRV_JOB}` job's `strategy:` {complaint}. \
                     `exclude:` removes a runner `os:` still lists and `include:` adds one it \
                     does not."
                ));
            }
            if field(strategy, "fail-fast").and_then(Yaml::as_bool) != Some(false) {
                out.push(format!(
                    "[msrv-job-matrix] the `{MSRV_JOB}` job's `fail-fast:` is not `false`, so \
                     one platform's floor failure cancels the other two before they report"
                ));
            }
            if let Some(matrix) = field(strategy, "matrix") {
                for complaint in field_complaints(matrix, &["os"], &[]) {
                    out.push(format!(
                        "[msrv-job-matrix] the `{MSRV_JOB}` job's `matrix:` {complaint}"
                    ));
                }
            }
        }
    }
    let listed = field(job, "strategy")
        .and_then(|strategy| field(strategy, "matrix"))
        .and_then(|matrix| field(matrix, "os"))
        .and_then(scalar_set);
    if listed.as_ref() != Some(&expected_runners) {
        out.push(format!(
            "[msrv-job-matrix] the `{MSRV_JOB}` job's matrix runs on {listed:?}, not exactly \
             {expected_runners:?}"
        ));
    }
    if scalar(job, "runs-on") != Some("${{ matrix.os }}") {
        out.push(format!(
            "[msrv-job-matrix] the `{MSRV_JOB}` job's `runs-on:` is {:?}, so the matrix above \
             decides nothing",
            scalar(job, "runs-on")
        ));
    }

    let toolchains: Vec<&Yaml> = steps_of(job)
        .iter()
        .filter(|step| {
            scalar(step, "uses").is_some_and(|uses| uses.starts_with("dtolnay/rust-toolchain@"))
        })
        .collect();
    if toolchains.len() != 1 {
        out.push(format!(
            "[msrv-job-toolchain] the `{MSRV_JOB}` job has {} `dtolnay/rust-toolchain` steps, \
             not one, so which toolchain checks the floor is not decidable here",
            toolchains.len()
        ));
        return out;
    }
    let selected = field(toolchains[0], "with").and_then(|with| scalar(with, "toolchain"));
    let expected = declared_msrv_toolchain();
    if selected != Some(expected.as_str()) {
        out.push(format!(
            "[msrv-job-toolchain] the `{MSRV_JOB}` job installs toolchain {selected:?}, not \
             `{expected}` -- the three-component form of `Cargo.toml`'s `rust-version`, which \
             is `{}`. A leg named for the floor that installs something else is green about a \
             version it never compiled.",
            declared_rust_version()
        ));
    }

    let install_at = steps_of(job).iter().position(|step| {
        scalar(step, "uses").is_some_and(|uses| uses.starts_with("dtolnay/rust-toolchain@"))
            && field(step, "with").and_then(|with| scalar(with, "toolchain"))
                == Some(expected.as_str())
    });
    let check_at = steps_of(job)
        .iter()
        .position(|step| scalar(step, "run") == Some(MSRV_COMMAND));
    if let Some((install_at, check_at)) = install_at
        .zip(check_at)
        .filter(|(install, check)| install > check)
    {
        out.push(format!(
            "[msrv-job-order] the `{MSRV_JOB}` job runs `{MSRV_COMMAND}` at step {check_at} \
             and installs toolchain `{expected}` at step {install_at}. The install selects the \
             toolchain for the steps that follow it, so a check above it compiles on whatever \
             the runner image shipped and the leg reports green about a version this crate \
             publishes no floor for."
        ));
    }
    out
}

pub(super) fn rustflags_complaints(doc: &Yaml) -> Vec<String> {
    let mut out = Vec::new();
    match field(doc, "env") {
        None => out.push(format!(
            "[rustflags] the workflow declares no `env:`, so nothing sets `{RUSTFLAGS_KEY}` at \
             workflow scope and no leg promotes a rustc warning to an error"
        )),
        Some(env) => {
            match field(env, RUSTFLAGS_KEY).map(Yaml::as_str) {
                None => out.push(format!(
                    "[rustflags] the workflow's `env:` does not bind `{RUSTFLAGS_KEY}`. \
                     `CODING_STANDARDS.md` §11 rests every leg's rustc-lint and \
                     `#[expect]`-retirement evidence on this one binding."
                )),
                Some(None) => out.push(format!(
                    "[rustflags] the workflow binds `{RUSTFLAGS_KEY}` to something YAML does \
                     not read as a string, so what the legs compile under is not something \
                     this contract can read -- and an unreadable value is not `{RUSTFLAGS_VALUE}`"
                )),
                Some(Some(found)) if found != RUSTFLAGS_VALUE => out.push(format!(
                    "[rustflags] the workflow binds `{RUSTFLAGS_KEY}` to `{found}`, not exactly \
                     `{RUSTFLAGS_VALUE}`. An equality rather than a `contains`: \
                     `-D warnings -A clippy::disallowed_methods` contains the pinned text and \
                     switches off the denylist this file exists to enforce."
                )),
                Some(Some(_)) => {}
            }
            for key in field_names(env) {
                let Some(guarded) = guarded_env_key(&key) else {
                    continue;
                };
                if key == RUSTFLAGS_KEY {
                    continue;
                }
                let why = if guarded == ENCODED_RUSTFLAGS_KEY {
                    format!(
                        "Cargo reads `{ENCODED_RUSTFLAGS_KEY}` in preference to \
                         `{RUSTFLAGS_KEY}` and ignores `{RUSTFLAGS_KEY}` entirely when it is \
                         set, so the pinned line is read past rather than obeyed"
                    )
                } else {
                    format!(
                        "`{key}` and `{RUSTFLAGS_KEY}` are one variable on `windows-latest`, \
                         where the process environment is case-insensitive, so which value \
                         the Windows legs compile under is not decided by this document"
                    )
                };
                out.push(format!(
                    "[rustflags] the workflow's `env:` binds `{key}` beside the pinned \
                     `{RUSTFLAGS_KEY}: {RUSTFLAGS_VALUE}`. {why}."
                ));
            }
        }
    }

    let Some(jobs) = field(doc, "jobs") else {
        return out;
    };
    for name in field_names(jobs) {
        let Some(job) = field(jobs, &name) else {
            continue;
        };
        out.extend(rustflags_override_complaints(job, &format!("job `{name}`")));
        for (index, step) in steps_of(job).iter().enumerate() {
            let named = format!("`{name}` step {index}");
            out.extend(rustflags_override_complaints(step, &named));
            if let Some(script) = scalar(step, "run") {
                out.extend(rustflags_script_complaints(script, &named));
            }
        }
    }
    out
}

fn guarded_env_key(key: &str) -> Option<&'static str> {
    [RUSTFLAGS_KEY, ENCODED_RUSTFLAGS_KEY]
        .into_iter()
        .find(|guarded| key.eq_ignore_ascii_case(guarded))
}

fn env_name_tokens(script: &str) -> BTreeSet<String> {
    script
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|token| !token.is_empty())
        .map(str::to_owned)
        .collect()
}

fn writes_the_job_env_file(script: &str) -> bool {
    let lowered = script.to_ascii_lowercase();
    lowered.contains("github_env") || lowered.contains("github.env")
}

fn rustflags_script_complaints(script: &str, named: &str) -> Vec<String> {
    let named_keys: BTreeSet<&'static str> = env_name_tokens(script)
        .iter()
        .filter_map(|token| guarded_env_key(token))
        .collect();
    let persists = writes_the_job_env_file(script);
    named_keys
        .into_iter()
        .map(|key| {
            if persists {
                format!(
                    "[rustflags-persisted] {named} names `{key}` in a script that writes the \
                     job-scoped environment file. A line written there binds the variable for \
                     every later step of this job, so the workflow-scope \
                     `{RUSTFLAGS_KEY}: {RUSTFLAGS_VALUE}` is narrowed without any `env:` \
                     mapping in this document saying so, and every field-set equality here \
                     still passes."
                )
            } else {
                format!(
                    "[rustflags-in-script] {named} names `{key}` in its script. The warning \
                     policy is set once, at workflow scope; a script that names it is either \
                     scoping different flags to its own command or persisting them, and this \
                     workflow has no leg that does either."
                )
            }
        })
        .collect()
}

fn rustflags_override_complaints(node: &Yaml, named: &str) -> Vec<String> {
    let Some(env) = field(node, "env") else {
        return Vec::new();
    };
    field_names(env)
        .into_iter()
        .filter_map(|key| guarded_env_key(&key).map(|guarded| (key, guarded)))
        .map(|(key, guarded)| {
            let case = if key == guarded {
                String::new()
            } else {
                format!(
                    " `{key}` and `{guarded}` are one variable on `windows-latest`, where the \
                     process environment is case-insensitive, so this binding is inert on the \
                     Unix legs and authoritative on the Windows one."
                )
            };
            format!(
                "[rustflags-override] {named} binds `{key}` in its own `env:`, which shadows \
                 the workflow-scope `{RUSTFLAGS_KEY}: {RUSTFLAGS_VALUE}` for everything that \
                 node covers.{case} `CODING_STANDARDS.md` §11 records that narrowing the \
                 workflow-scope setting takes the `#[expect]` self-retirement guarantee with \
                 it, silently."
            )
        })
        .collect()
}

pub(super) fn workflow_complaints(doc: &Yaml) -> Vec<String> {
    let mut out = ci_gate_complaints(doc);
    out.extend(ci_test_job_complaints(doc));
    out.extend(ci_test_windows_job_complaints(doc));
    out.extend(ci_test_coverage_job_complaints(doc));
    out.extend(ci_windows_build_witness_complaints(doc));
    out.extend(workflow_env_complaints(doc));
    out.extend(workflow_permissions_complaints(doc));
    out.extend(ci_msrv_job_complaints(doc));
    out.extend(rustflags_complaints(doc));
    out
}

pub(super) fn complaint_codes(complaints: &[String]) -> BTreeSet<String> {
    complaints
        .iter()
        .filter_map(|complaint| {
            complaint
                .strip_prefix('[')
                .and_then(|rest| rest.split(']').next())
                .map(str::to_owned)
        })
        .collect()
}

pub(super) struct WorkflowEscape {
    pub(super) name: &'static str,
    pub(super) escape: &'static str,
    pub(super) job: Option<&'static str>,
    pub(super) anchor: &'static str,
    pub(super) replacement: &'static str,
    pub(super) refused_as: &'static str,
}

pub(super) const WORKFLOW_ESCAPES: &[WorkflowEscape] = &[
    WorkflowEscape {
        name: "MUT-GATE-ECHOED",
        escape: "the job echoes the gate command, succeeds, and the aggregate settles green \
                 while Clippy never examines a denied call in that platform's code. The \
                 standing row's first escape, verbatim.",
        job: Some("lint-macos"),
        anchor: "      - run: cargo clippy --all-targets --all-features -- -D warnings\n",
        replacement: "      - run: echo cargo clippy --all-targets --all-features -- -D warnings\n",
        refused_as: "no-gate-job",
    },
    WorkflowEscape {
        name: "MUT-GATE-JOB-DISABLED",
        escape: "a job-level `if: false` reports success without running the gate",
        job: Some("lint-windows"),
        anchor: "    runs-on: windows-latest\n",
        replacement: "    runs-on: windows-latest\n    if: false\n",
        refused_as: "unexpected-job-field",
    },
    WorkflowEscape {
        name: "MUT-GATE-JOB-DISABLED-EXPRESSION",
        escape: "the same disabling written as an expression. The predecessor searched the \
                 whitespace-normalised block for the literal `if: false` and could not refuse \
                 this form; the field-set equality does not have to know the form.",
        job: Some("lint-macos"),
        anchor: "    runs-on: macos-latest\n",
        replacement: "    runs-on: macos-latest\n    if: ${{ false }}\n",
        refused_as: "unexpected-job-field",
    },
    WorkflowEscape {
        name: "MUT-GATE-JOB-ABSOLVED",
        escape: "`continue-on-error` at job level: the gate runs, fails, and reports success",
        job: Some("lint"),
        anchor: "    runs-on: ubuntu-latest\n",
        replacement: "    runs-on: ubuntu-latest\n    continue-on-error: true\n",
        refused_as: "unexpected-job-field",
    },
    WorkflowEscape {
        name: "MUT-GATE-STEP-ABSOLVED",
        escape: "the same absolution one level down, on the step that runs the gate",
        job: Some("lint-windows"),
        anchor: "      - run: cargo clippy --all-targets --all-features -- -D warnings\n",
        replacement: "      - run: cargo clippy --all-targets --all-features -- -D warnings\n\
                      \x20       continue-on-error: true\n",
        refused_as: "unexpected-step-field",
    },
    WorkflowEscape {
        name: "MUT-GATE-STEP-RETARGETED",
        escape: "a step-level environment retargets the compile. The `run:` scalar still \
                 matches character for character, the job is green, and Clippy examined a \
                 target whose `#[cfg]` bodies are not this platform's.",
        job: Some("lint-macos"),
        anchor: "      - run: cargo clippy --all-targets --all-features -- -D warnings\n",
        replacement: "      - run: cargo clippy --all-targets --all-features -- -D warnings\n\
                      \x20       env:\n\
                      \x20         CARGO_BUILD_TARGET: x86_64-unknown-linux-gnu\n",
        refused_as: "unexpected-step-field",
    },
    WorkflowEscape {
        name: "MUT-GATE-STEP-CUSTOM-SHELL",
        escape: "a custom shell. GitHub runs `<template> <script file>`, so `shell: 'true'` \
                 runs `true /path/to/script`: the step succeeds, Clippy never runs, and the \
                 `run:` scalar still matches this contract character for character.",
        job: Some("lint-macos"),
        anchor: "      - run: cargo clippy --all-targets --all-features -- -D warnings\n",
        replacement: "      - run: cargo clippy --all-targets --all-features -- -D warnings\n\
                      \x20       shell: 'true'\n",
        refused_as: "step-shell",
    },
    WorkflowEscape {
        name: "MUT-WORKFLOW-DEFAULT-SHELL-SWAPPED",
        escape: "a workflow-level `defaults.run.shell` swaps the interpreter under every \
                 `run:` step at once, and no step declares anything for a step-only reading \
                 to notice. The aggregate's bash script is the one that breaks loudest; the \
                 gates are the ones that break quietly.",
        job: None,
        anchor: "\njobs:\n",
        replacement: "\ndefaults:\n  run:\n    shell: pwsh\n\njobs:\n",
        refused_as: "step-shell",
    },
    WorkflowEscape {
        name: "MUT-TEST-MATRIX-EXCLUDED",
        escape: "`exclude:` beside the matrix's `include:` list. GitHub processes `include:` \
                 after `exclude:`, so this one removes nothing today, but every check that reads \
                 `include:` would pass over the leg an `exclude:` drops on the day it applies -- \
                 the matrix half of `PR5-MACOS-CLIPPY-NEVER-RUN`. The contract models the one \
                 list it reads and refuses any key beside it.",
        job: Some("test"),
        anchor: "\x20       include:\n",
        replacement: "\x20       exclude:\n\
                      \x20         - os: macos-latest\n\
                      \x20       include:\n",
        refused_as: "test-job-matrix",
    },
    WorkflowEscape {
        name: "MUT-AGGREGATE-STEM-COLLISION",
        escape: "two job ids that normalise to one gate variable stem. Every collection the \
                 aggregate's checks derive from stems is a set or a map, so the collision \
                 collapses both the expectation and the reality and the equalities compare \
                 equal -- one of the two gates is then unreadable and unrequired.",
        job: None,
        anchor: "  merge-gate:\n",
        replacement: "  lint_windows:\n\
                      \x20   name: lint (collision)\n\
                      \x20   runs-on: ubuntu-latest\n\
                      \x20   timeout-minutes: 5\n\
                      \x20   steps:\n\
                      \x20     - run: echo collision\n\
                      \n\
                      \x20 merge-gate:\n",
        refused_as: "aggregate-stem-collision",
    },
    WorkflowEscape {
        name: "MUT-GATE-RUNNER-DRIFT",
        escape: "the macOS leg moved to Ubuntu: two jobs run the gate on one platform and no \
                 job compiles `#[cfg(target_os = \"macos\")]` under Clippy. \
                 `PR5-MACOS-CLIPPY-NEVER-RUN` is the ledger row for the state this restores.",
        job: Some("lint-macos"),
        anchor: "    runs-on: macos-latest\n",
        replacement: "    runs-on: ubuntu-latest\n",
        refused_as: "no-gate-job",
    },
    WorkflowEscape {
        name: "MUT-AGGREGATE-NEEDS-DROPPED",
        escape: "a gate the aggregate does not depend on: branch protection settles green while \
                 the Windows denial gate is red. `PR5D-MSVC-CLIPPY-NEVER-RUN`.",
        job: Some("merge-gate"),
        anchor: "\x20   needs: [lint, lint-windows, lint-macos, msrv, test, test-windows, test-coverage]\n",
        replacement: "\x20   needs: [lint, lint-macos, msrv, test, test-windows, test-coverage]\n",
        refused_as: "aggregate-needs",
    },
    WorkflowEscape {
        name: "MUT-AGGREGATE-ENV-DECOY",
        escape: "a binding that names one gate and reads another's result. It satisfies any \
                 existence check, reads a passing sibling, and with `always()` on the \
                 aggregate reports the required check green over a red leaf. Measured: the \
                 predecessor of this assertion accepted exactly this.",
        job: Some("merge-gate"),
        anchor: "          LINT_MACOS_RESULT: ${{ needs.lint-macos.result }}\n",
        replacement: "          LINT_MACOS_RESULT: ${{ needs.lint-windows.result }}\n",
        refused_as: "aggregate-env",
    },
    WorkflowEscape {
        name: "MUT-AGGREGATE-LOOP-DECOY",
        escape: "the loop omits a gate while the omitted name still appears on the line. The \
                 shape the predecessor's doc enumerated as the escape it could not close: \
                 `requires.split_whitespace().any(|word| word == looped)` passes on the trailing \
                 mention.",
        job: Some("merge-gate"),
        anchor: "\x20         for gate in LINT LINT_WINDOWS LINT_MACOS MSRV TEST TEST_WINDOWS TEST_COVERAGE; do\n",
        replacement: "\x20         for gate in LINT LINT_WINDOWS MSRV TEST TEST_WINDOWS TEST_COVERAGE; do : LINT_MACOS\n",
        refused_as: "aggregate-loop",
    },
    WorkflowEscape {
        name: "MUT-AGGREGATE-ALWAYS-DROPPED",
        escape: "without `always()` a failed dependency skips the aggregate, and a skipped \
                 required check never settles at all",
        job: Some("merge-gate"),
        anchor: "    if: always()\n",
        replacement: "    if: success()\n",
        refused_as: "aggregate-condition",
    },
    WorkflowEscape {
        name: "MUT-AGGREGATE-CONTEXT-RENAMED",
        escape: "branch protection requires one context name; a renamed job publishes a \
                 different one and the required check waits forever",
        job: Some("merge-gate"),
        anchor: "    name: upstroke-ci\n",
        replacement: "    name: upstroke-ci-aggregate\n",
        refused_as: "aggregate-context-name",
    },
    WorkflowEscape {
        name: "MUT-DUPLICATE-KEY",
        escape: "a second `runs-on:` in the same job. Under a last-one-wins parser every \
                 equality in this section reads the winner and the mutation hides in the \
                 loser; this is the property the dependency was chosen for, executed.",
        job: Some("lint-windows"),
        anchor: "    runs-on: windows-latest\n",
        replacement: "    runs-on: windows-latest\n    runs-on: ubuntu-latest\n",
        refused_as: "parse",
    },
    WorkflowEscape {
        name: "MUT-CI-CARGO-TEST-STEP-DELETED",
        escape: "the job that runs these fixtures stops running them. Named in \
                 `test-docs-consistency.sh` as a mutation that gate's withdrawn claim could \
                 not kill.",
        job: Some("test"),
        anchor: "      - run: cargo test --all-targets --all-features\n",
        replacement: "",
        refused_as: "test-job-command",
    },
    WorkflowEscape {
        name: "MUT-CI-CARGO-TEST-STEP-SKIPPED",
        escape: "the same, by disabling the job rather than deleting the step. The other \
                 mutation `test-docs-consistency.sh` kept as history.",
        job: Some("test"),
        anchor: "    runs-on: ${{ matrix.os }}\n",
        replacement: "    runs-on: ${{ matrix.os }}\n    if: false\n",
        refused_as: "unexpected-job-field",
    },
    WorkflowEscape {
        name: "MUT-CI-STOPS-INSTALLING-CLIPPY",
        escape: "`clippy-driver` is a test dependency of the job that runs these fixtures. \
                 The first version of that test looked for the word `clippy` in the job's \
                 text and the nine-line comment above the line spelled it, so deleting the \
                 line left the test green -- `PR4-CENSUS-COMMENT-ORACLE`, in the test whose \
                 purpose is to answer which command runs this.",
        job: Some("test"),
        anchor: "          components: clippy\n",
        replacement: "",
        refused_as: "test-job-toolchain",
    },
    WorkflowEscape {
        name: "MUT-CI-TEST-MATRIX-NARROWED",
        escape: "the hosted fixtures run on one platform and the other goes unexercised, while \
                 every check that names the command still passes: the three macOS shards dropped \
                 from the matrix",
        job: Some("test"),
        anchor: "\x20         - os: macos-latest\n\
                 \x20           shard: m1\n\
                 \x20         - os: macos-latest\n\
                 \x20           shard: m2\n\
                 \x20         - os: macos-latest\n\
                 \x20           shard: m3\n",
        replacement: "",
        refused_as: "test-job-matrix",
    },
    WorkflowEscape {
        name: "MUT-TEST-MATRIX-KEEPS-WINDOWS",
        escape: "the hosted matrix quietly re-admits Windows. Two jobs then run the Windows \
                 suite, and the one whose duration this contract retired is back on the critical \
                 path with every other check passing.",
        job: Some("test"),
        anchor: "\x20       include:\n",
        replacement: "\x20       include:\n\
                      \x20         - os: windows-latest\n\
                      \x20           shard: all\n",
        refused_as: "test-job-matrix",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-REHOSTED",
        escape: "every leg on `windows-latest`, as a scalar `runs-on:`. Every step still matches \
                 character for character and the install step's condition still holds on the \
                 shards; only the guest's leg changed machine, and with it the six minutes every \
                 pull request waits on became twenty-five. The install is skipped on that leg, so \
                 its compiler check throws on every pull request: loud, but a workflow that is \
                 red by construction is refused here before it is pushed.",
        job: Some("test-windows"),
        anchor: "\x20   runs-on: ${{ matrix.shard != 'all' && 'windows-latest' || fromJSON('[\"self-hosted\", \"windows\", \"winguest\"]') }}\n",
        replacement: "\x20   runs-on: windows-latest\n",
        refused_as: "test-windows-runner",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-LABEL-DROPPED",
        escape: "the self-hosted labels loosened to `[self-hosted, windows]`, which any Windows \
                 runner the account ever registers satisfies; the curated image is named by the \
                 label this drops",
        job: Some("test-windows"),
        anchor: "\x20   runs-on: ${{ matrix.shard != 'all' && 'windows-latest' || fromJSON('[\"self-hosted\", \"windows\", \"winguest\"]') }}\n",
        replacement: "\x20   runs-on: ${{ matrix.shard != 'all' && 'windows-latest' || fromJSON('[\"self-hosted\", \"windows\"]') }}\n",
        refused_as: "test-windows-runner",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-QUEUE-LANE-REGUESTED",
        escape: "the expression replaced by the plain label set. Every leg lands on the \
                 pull-request guest, the merge queue's four shards queue behind whatever \
                 pull-request build holds it, and the shards' install step, conditioned on the \
                 shard rather than on the machine, now runs on the guest and installs over the \
                 image's compiler, under the hosted lane's ceiling.",
        job: Some("test-windows"),
        anchor: "\x20   runs-on: ${{ matrix.shard != 'all' && 'windows-latest' || fromJSON('[\"self-hosted\", \"windows\", \"winguest\"]') }}\n",
        replacement: "\x20   runs-on: [self-hosted, windows, winguest]\n",
        refused_as: "test-windows-runner",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-LANES-SWAPPED",
        escape: "the condition inverted in `runs-on:` alone: the guest's whole-suite leg goes to \
                 `windows-latest`, where no step installs a compiler, and every shard goes to the \
                 guest, where the install step now runs over the image's own. The install's `if:` \
                 still matches; it is the runner that no longer agrees with it.",
        job: Some("test-windows"),
        anchor: "\x20   runs-on: ${{ matrix.shard != 'all' && 'windows-latest' || fromJSON('[\"self-hosted\", \"windows\", \"winguest\"]') }}\n",
        replacement: "\x20   runs-on: ${{ matrix.shard == 'all' && 'windows-latest' || fromJSON('[\"self-hosted\", \"windows\", \"winguest\"]') }}\n",
        refused_as: "test-windows-runner",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-COMMAND-DELETED",
        escape: "the self-hosted job checks out, configures git, and runs no suite. The \
                 witness makes this one loud twice over -- the step is off its pin, and a \
                 count of nothing is below the floor -- but the pin is what refuses it here.",
        job: Some("test-windows"),
        anchor: "          cargo test --all-targets --all-features | Tee-Object -Variable log\n",
        replacement: "",
        refused_as: "test-windows-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-DISABLED",
        escape: "the whole-suite step's condition becomes `false`: the guest's leg reports \
                 success having run nothing. A job-level `if:` is not this escape -- the job \
                 reports `skipped` and the aggregate, which accepts only `success`, fails.",
        job: Some("test-windows"),
        anchor: "\x20     - name: Test, and witness that the suite ran\n\
                 \x20       if: matrix.shard == 'all'\n\
                 \x20       env:\n\
                 \x20         UPSTROKE_TEST_TEMP_FOLDS_CASE: \"1\"\n",
        replacement: "\x20     - name: Test, and witness that the suite ran\n\
                      \x20       if: false\n\
                      \x20       env:\n\
                      \x20         UPSTROKE_TEST_TEMP_FOLDS_CASE: \"1\"\n",
        refused_as: "test-windows-suite-lane",
    },
    WorkflowEscape {
        name: "MUT-WINDOWS-WITNESS-COUNT-DROPPED",
        escape: "the suite step keeps the pinned command and loses the count, which is the \
                 shape this leg had before the witness and the shape `master` still has. A \
                 `target.<triple>.runner` in the guest's Cargo home or environment then hands \
                 every compiled harness to a wrapper that exits zero, the job is green, and \
                 nothing on the hosted side can see the machine it happened on.",
        job: Some("test-windows"),
        anchor: "          $passed = [int](($log | Select-String -Pattern '^test result: ok\\. (\\d+) passed' | ForEach-Object { [int]$_.Matches[0].Groups[1].Value } | Measure-Object -Sum).Sum)\n          if ($passed -lt 1700) { throw \"the suite reported $passed passing tests, below the floor of 1700: Cargo compiled the harnesses and executed almost none of them\" }\n",
        replacement: "",
        refused_as: "test-windows-command",
    },
    WorkflowEscape {
        name: "MUT-WINDOWS-WITNESS-COMPILER-CHECK-DROPPED",
        escape: "the two lines that refuse any compiler but the image's are gone from the \
                 whole-suite step and the suite runs on whatever Cargo resolves to. The install \
                 step still matches whole, the toolchain action tolerates a failed `rustup \
                 default` and verifies only `rustc +1.97.1`, and a `RUSTC` in the guest's service \
                 environment is read by nothing here, so either lane can run the suite on a \
                 compiler the other never saw with the install reported green -- the one-compiler \
                 claim with nothing holding it.",
        job: Some("test-windows"),
        anchor: "\x20         $rustc = ((cargo rustc -q --lib -- --version) | Out-String).Trim(); $path_rustc = ((rustc --version) | Out-String).Trim(); $cargo = ((cargo --version) | Out-String).Trim()\n\
                 \x20         if ($rustc -notmatch '^rustc 1\\.97\\.1 ' -or $path_rustc -notmatch '^rustc 1\\.97\\.1 ' -or $cargo -notmatch '^cargo 1\\.97\\.1 ') { throw \"the suite would run on '$rustc' (Cargo's compiler), '$path_rustc' (PATH's rustc) and '$cargo', not the image's 1.97.1: a compiler was chosen by something other than this workflow\" }\n\
                 \x20         cargo test --all-targets --all-features | Tee-Object -Variable log\n",
        replacement: "\x20         cargo test --all-targets --all-features | Tee-Object -Variable log\n",
        refused_as: "test-windows-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-CACHE-BEFORE-CHECKOUT",
        escape: "a pinned cache step placed above the checkout, everything else in order: the \
                 checkout slips to step 1 and the install to step 2, so the install's position \
                 check refuses this shape too. The checkout pin is named because it is the one \
                 that reads the cause -- a Rust-aware action running before the tree exists -- \
                 and `MUT-TEST-WINDOWS-CHECKOUT-AFTER-INSTALL` is the shape only it refuses.",
        job: Some("test-windows"),
        anchor: "      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0\n\n\
                 \x20     # The hosted lane's compiler: the image's, exactly. The workflow oracle\n",
        replacement: "      - uses: Swatinem/rust-cache@6323deb102c322ba6fcbdcafc7e3dddab59af2b6 # v2.9.2\n\
                      \x20     - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0\n\n\
                      \x20     # The hosted lane's compiler: the image's, exactly. The workflow oracle\n",
        refused_as: "test-windows-checkout",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-CHECKOUT-AFTER-INSTALL",
        escape: "the shape the checkout pin exists for: a pinned cache step at 0, the install \
                 still at 1, and the checkout moved to 2. The install's position check passes, \
                 the checkout is still input-free, and a Rust-aware action runs before the \
                 compiler is selected and before the tree exists; only a pin on where the \
                 checkout is, not merely what it is given, refuses it.",
        job: Some("test-windows"),
        anchor: "\x20     - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0\n\n\
                 \x20     # The hosted lane's compiler: the image's, exactly. The workflow oracle\n\
                 \x20     # pins this step whole -- one install, right after the checkout, on\n\
                 \x20     # exactly the legs whose runner needs one, at exactly the image's\n\
                 \x20     # version. On the guest's leg the step is skipped after its archive is\n\
                 \x20     # downloaded at setup.\n\
                 \x20     - name: Install the image's toolchain on the hosted lane\n\
                 \x20       if: matrix.shard != 'all'\n\
                 \x20       uses: dtolnay/rust-toolchain@4360b52568e2003a75bf9bc1d59f33a8e3fc893c # stable\n\
                 \x20       with:\n\
                 \x20         toolchain: 1.97.1\n\
                 \x20         components: clippy\n",
        replacement: "\x20     - uses: Swatinem/rust-cache@6323deb102c322ba6fcbdcafc7e3dddab59af2b6 # v2.9.2\n\n\
                      \x20     # The hosted lane's compiler: the image's, exactly. The workflow oracle\n\
                      \x20     # pins this step whole -- one install, right after the checkout, on\n\
                      \x20     # exactly the legs whose runner needs one, at exactly the image's\n\
                      \x20     # version. On the guest's leg the step is skipped after its archive is\n\
                      \x20     # downloaded at setup.\n\
                      \x20     - name: Install the image's toolchain on the hosted lane\n\
                      \x20       if: matrix.shard != 'all'\n\
                      \x20       uses: dtolnay/rust-toolchain@4360b52568e2003a75bf9bc1d59f33a8e3fc893c # stable\n\
                      \x20       with:\n\
                      \x20         toolchain: 1.97.1\n\
                      \x20         components: clippy\n\
                      \x20     - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0\n",
        refused_as: "test-windows-checkout",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-SECOND-CHECKOUT",
        escape: "a second, input-free checkout after the identity step. The first still \
                 matches; the second re-checks out the event's ref, which for this job is the \
                 same tree today and is whatever the action's default becomes tomorrow, and \
                 anything the identity step wrote to the tree is gone.",
        job: Some("test-windows"),
        anchor: "      - name: Test, and witness that the suite ran\n",
        replacement: "      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0\n\
                      \x20     - name: Test, and witness that the suite ran\n",
        refused_as: "test-windows-checkout",
    },
    WorkflowEscape {
        name: "MUT-WINDOWS-WITNESS-FLOOR-DROPPED",
        escape: "the count stays and its floor drops to zero, so a suite that executed no \
                 test clears it. The step still reads as a witness; it witnesses nothing. \
                 Only an equality over the whole script sees the number change.",
        job: Some("test-windows"),
        anchor: "          if ($passed -lt 1700) { throw \"the suite reported $passed passing tests, below the floor of 1700: Cargo compiled the harnesses and executed almost none of them\" }\n",
        replacement: "          if ($passed -lt 0) { throw \"the suite reported $passed passing tests, below the floor of 1700: Cargo compiled the harnesses and executed almost none of them\" }\n",
        refused_as: "test-windows-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-TOOLCHAIN-BEHIND-THE-IMAGE",
        escape: "the hosted lane installs `1.96.0`, a compiler the image never carried. Every \
                 step-level pin still holds -- the action is allowlisted, its input keys are \
                 allowlisted, the components value is pinned, the condition is the lane's. \
                 With the suite step intact its compiler check throws on every queue entry, \
                 so this is not a silent escape but a drift between the two places the \
                 version is pinned, and the oracle refuses the drift rather than letting the \
                 queue discover it. Edited together with the suite step it would be silent: \
                 the queue runs a compiler the pull-request lane never saw, and a Windows test \
                 enabled only on 1.97 and later is omitted on the lane that lands the merge. \
                 Before the hosted lane existed this was the ninth review pass's finding: the \
                 hosted jobs pinned `stable` through `toolchain_complaints`, and this job, \
                 which installed nothing, was never asked.",
        job: Some("test-windows"),
        anchor: "          toolchain: 1.97.1\n",
        replacement: "          toolchain: 1.96.0\n",
        refused_as: "test-windows-toolchain",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-TOOLCHAIN-FLOATS",
        escape: "the hosted lane installs `stable`, which is already ahead of the image's \
                 compiler. With the suite step intact its compiler check throws on every \
                 queue entry; edited together with it, the queue runs the suite on a compiler \
                 the pull-request lane never ran it on, so a test whose outcome depends on the \
                 compiler -- a version-gated test, a std behaviour change, a new lint in test \
                 code under `-D warnings` -- passes on the guest and fails in the queue, or \
                 the reverse, for a reason unrelated to the diff. The pin is what keeps the \
                 two test lanes one compiler, and the oracle refuses the drift between the \
                 install and the suite step's check either way.",
        job: Some("test-windows"),
        anchor: "          toolchain: 1.97.1\n",
        replacement: "          toolchain: stable\n",
        refused_as: "test-windows-toolchain",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-INSTALL-UNCONDITIONAL",
        escape: "the install step's `if:` dropped. The shards are unchanged; the guest's leg now \
                 has its compiler selected by the workflow instead of by the image, which is the \
                 escape the contract refused before there was a hosted lane -- `rustup` on a \
                 frozen image, over a curated toolchain, on every pull request.",
        job: Some("test-windows"),
        anchor: "\x20     - name: Install the image's toolchain on the hosted lane\n\
                 \x20       if: matrix.shard != 'all'\n",
        replacement: "\x20     - name: Install the image's toolchain on the hosted lane\n",
        refused_as: "test-windows-toolchain",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-INSTALL-ON-THE-WRONG-LANE",
        escape: "the install step's condition inverted while `runs-on:` keeps its legs: the \
                 shards are left with whatever GitHub's image preinstalled -- current stable, or \
                 the previous one during a rollout -- where the shard step's compiler check then \
                 throws on every queue entry, and the guest's leg installs over its own. Both \
                 pins are exact; they no longer name the same legs, and a workflow whose queue is \
                 red by construction is refused before it is pushed.",
        job: Some("test-windows"),
        anchor: "\x20     - name: Install the image's toolchain on the hosted lane\n\
                 \x20       if: matrix.shard != 'all'\n",
        replacement: "\x20     - name: Install the image's toolchain on the hosted lane\n\
                      \x20       if: matrix.shard == 'all'\n",
        refused_as: "test-windows-toolchain",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-COMPONENTS-WITHOUT-CLIPPY",
        escape: "the hosted lane's install drops `components: clippy`. The action installs \
                 the minimal profile, so `clippy-driver` is absent and the effect-denial \
                 fixtures cannot run on the lane that lands the merge -- \
                 `MUT-CI-STOPS-INSTALLING-CLIPPY`'s shape on this job.",
        job: Some("test-windows"),
        anchor: "          components: clippy\n",
        replacement: "",
        refused_as: "test-windows-toolchain",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-SECOND-INSTALL",
        escape: "a second, unconditional toolchain step at `stable` after the identity step. \
                 The pinned install is untouched, so every equality over it holds; the second \
                 install makes `stable` the default on both lanes, where the suite step's \
                 compiler check then throws on every build. Loud today; the oracle refuses two \
                 installs because which compiler the job runs must be decided by one step it \
                 pins whole, not by whichever install ran last.",
        job: Some("test-windows"),
        anchor: "      - name: Test, and witness that the suite ran\n",
        replacement: "      - uses: dtolnay/rust-toolchain@4360b52568e2003a75bf9bc1d59f33a8e3fc893c # stable\n\
                      \x20       with:\n\
                      \x20         toolchain: stable\n\
                      \x20         components: clippy\n\
                      \x20     - name: Test, and witness that the suite ran\n",
        refused_as: "test-windows-toolchain",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-INSTALL-NOT-FIRST",
        escape: "the install step moved below the identity step. Nothing runs Cargo between the \
                 two today, so this is the shape `MUT-TOOLCHAIN-NOT-FIRST` refuses on the gates, \
                 held here for the same reason: a Cargo command placed above the install would \
                 run on GitHub's preinstalled compiler with every other pin matching.",
        job: Some("test-windows"),
        anchor: "\x20     - name: Install the image's toolchain on the hosted lane\n\
                 \x20       if: matrix.shard != 'all'\n\
                 \x20       uses: dtolnay/rust-toolchain@4360b52568e2003a75bf9bc1d59f33a8e3fc893c # stable\n\
                 \x20       with:\n\
                 \x20         toolchain: 1.97.1\n\
                 \x20         components: clippy\n\n\
                 \x20     - name: Configure git identity\n\
                 \x20       run: |\n\
                 \x20         git config --global user.email \"ci@upstroke.local\"\n\
                 \x20         git config --global user.name \"upstroke CI\"\n",
        replacement: "\x20     - name: Configure git identity\n\
                      \x20       run: |\n\
                      \x20         git config --global user.email \"ci@upstroke.local\"\n\
                      \x20         git config --global user.name \"upstroke CI\"\n\n\
                      \x20     - name: Install the image's toolchain on the hosted lane\n\
                      \x20       if: matrix.shard != 'all'\n\
                      \x20       uses: dtolnay/rust-toolchain@4360b52568e2003a75bf9bc1d59f33a8e3fc893c # stable\n\
                      \x20       with:\n\
                      \x20         toolchain: 1.97.1\n\
                      \x20         components: clippy\n",
        refused_as: "test-windows-toolchain",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-SUITE-ON-ONE-LANE",
        escape: "the whole-suite step also conditioned on the merge-queue event, which never runs \
                 the guest's leg. The pull-request lane checks out, configures git, and reports \
                 success having run nothing; the queue still runs its shards, so the merge is \
                 still guarded, and every pull request is green on Windows for free. The \
                 allowance is per step and per condition, not per value.",
        job: Some("test-windows"),
        anchor: "\x20     - name: Test, and witness that the suite ran\n\
                 \x20       if: matrix.shard == 'all'\n\
                 \x20       env:\n\
                 \x20         UPSTROKE_TEST_TEMP_FOLDS_CASE: \"1\"\n",
        replacement: "\x20     - name: Test, and witness that the suite ran\n\
                      \x20       if: matrix.shard == 'all' && github.event_name == 'merge_group'\n\
                      \x20       env:\n\
                      \x20         UPSTROKE_TEST_TEMP_FOLDS_CASE: \"1\"\n",
        refused_as: "test-windows-suite-lane",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-ONLY-RETARGET",
        escape: "a `run:` step under the lane condition that fetches `master` and checks it \
                 out before the suite. The pull-request lane, the one a reviewer watches, \
                 never executes it; the queue tests `master` and lands the candidate. Refused \
                 twice, by the field set and by the script pin, and the field set is the one \
                 named here because it refuses the lane-conditional script before reading it.",
        job: Some("test-windows"),
        anchor: "      - name: Test, and witness that the suite ran\n",
        replacement: "      - if: github.event_name == 'merge_group'\n\
                      \x20       run: git fetch origin master && git checkout --detach FETCH_HEAD\n\
                      \x20     - name: Test, and witness that the suite ran\n",
        refused_as: "unexpected-step-field",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-RENAMED-AWAY",
        escape: "the job renamed: the pinned name has no job. Not a false green -- the \
                 aggregate's derived `needs` refuses the renamed workflow too -- but a lost \
                 handle: every pin above hangs from this name, so the contract must notice the \
                 name going before it notices nothing else",
        job: None,
        anchor: "\n  test-windows:\n",
        replacement: "\n  test-windows-hosted:\n",
        refused_as: "test-windows-missing",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-RUN-RETARGETED",
        escape: "the identity step of the self-hosted job fetches `master` and checks it out \
                 detached before the pinned test command runs. Labels, fields, shell, the \
                 input-free checkout and the exact command all still match; the suite tests \
                 `master`.",
        job: Some("test-windows"),
        anchor: "          git config --global user.name \"upstroke CI\"\n",
        replacement: "          git config --global user.name \"upstroke CI\"\n\
                      \x20         git fetch origin master\n\
                      \x20         git checkout --detach FETCH_HEAD\n",
        refused_as: "test-windows-run-script",
    },
    WorkflowEscape {
        name: "MUT-TEST-RUN-RETARGETED",
        escape: "the same retarget on the hosted test matrix: two platforms test `master` \
                 while every pin still matches",
        job: Some("test"),
        anchor: "          git config --global user.name \"upstroke CI\"\n",
        replacement: "          git config --global user.name \"upstroke CI\"\n\
                      \x20         git fetch origin master\n\
                      \x20         git checkout --detach FETCH_HEAD\n",
        refused_as: "test-job-run-script",
    },
    WorkflowEscape {
        name: "MUT-TEST-CASEFOLD-DECLARATION-DROPPED",
        escape: "the hosted whole-suite step no longer declares the leg's casefold, so on a leg \
                 whose temporary directory folds case the alias test observes its native branch \
                 instead of requiring it: a folding temporary directory is no longer a \
                 prerequisite of the leg, and on a temporary directory that does not fold case \
                 the undeclared test passes with the branch unrun where the declared one fails at \
                 the declaration (34-r6-casefold-declaration.log, ext4) -- while every pin still \
                 matches",
        job: Some("test"),
        anchor: "\x20       if: matrix.shard == 'all'\n\
                 \x20       env:\n\
                 \x20         UPSTROKE_TEST_TEMP_FOLDS_CASE: ${{ matrix.os == 'macos-latest' && '1' || '' }}\n",
        replacement: "\x20       if: matrix.shard == 'all'\n",
        refused_as: "test-step-env",
    },
    WorkflowEscape {
        name: "MUT-TEST-CASEFOLD-DECLARED-ON-THE-WRONG-LEG",
        escape: "the declaration moves to the ubuntu runner, whose temporary directory does not \
                 fold case: the macOS legs observe instead of requiring, and the ubuntu leg fails \
                 its declaration loudly -- a red that reads as a flake rather than as the guard \
                 having moved",
        job: Some("test"),
        anchor: "\x20       if: matrix.shard == 'all'\n\
                 \x20       env:\n\
                 \x20         UPSTROKE_TEST_TEMP_FOLDS_CASE: ${{ matrix.os == 'macos-latest' && '1' || '' }}\n",
        replacement: "\x20       if: matrix.shard == 'all'\n\
                      \x20       env:\n\
                      \x20         UPSTROKE_TEST_TEMP_FOLDS_CASE: ${{ matrix.os == 'ubuntu-latest' && '1' || '' }}\n",
        refused_as: "test-step-env",
    },
    WorkflowEscape {
        name: "MUT-TEST-STEP-RETARGETED-THROUGH-THE-ADMITTED-ENV",
        escape: "the hosted test step's `env:`, one of the step-level maps this contract admits, \
                 gains a second key: `CARGO_BUILD_TARGET` retargets the compile the suite \
                 performs while the `run:` scalar and the declaration both still match -- \
                 `MUT-GATE-STEP-RETARGETED`'s shape on the step that now carries an environment",
        job: Some("test"),
        anchor: "\x20       if: matrix.shard == 'all'\n\
                 \x20       env:\n\
                 \x20         UPSTROKE_TEST_TEMP_FOLDS_CASE: ${{ matrix.os == 'macos-latest' && '1' || '' }}\n",
        replacement: "\x20       if: matrix.shard == 'all'\n\
                      \x20       env:\n\
                      \x20         UPSTROKE_TEST_TEMP_FOLDS_CASE: ${{ matrix.os == 'macos-latest' && '1' || '' }}\n\
                      \x20         CARGO_BUILD_TARGET: x86_64-unknown-linux-gnu\n",
        refused_as: "test-step-env",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-CASEFOLD-DECLARATION-DROPPED",
        escape: "the self-hosted step no longer declares that the guest's NTFS folds case, so the \
                 alias test observes its native branch on Windows instead of requiring it, with \
                 the witness script and every label still matching",
        job: Some("test-windows"),
        anchor: "\x20     - name: Test, and witness that the suite ran\n\
                 \x20       if: matrix.shard == 'all'\n\
                 \x20       env:\n\
                 \x20         UPSTROKE_TEST_TEMP_FOLDS_CASE: \"1\"\n",
        replacement: "\x20     - name: Test, and witness that the suite ran\n\
                      \x20       if: matrix.shard == 'all'\n",
        refused_as: "test-windows-step-env",
    },
    WorkflowEscape {
        name: "MUT-GATE-RUN-RETARGETED",
        escape: "a step ahead of the Windows Clippy gate and the build witness checks out \
                 `master`; both pinned commands then run against a tree the candidate never \
                 touched, and a Windows-only denial or link failure in the candidate goes green",
        job: Some("lint-windows"),
        anchor: "      - run: cargo clippy --all-targets --all-features -- -D warnings\n",
        replacement: "      - run: git fetch origin master && git checkout --detach FETCH_HEAD\n\
                      \x20     - run: cargo clippy --all-targets --all-features -- -D warnings\n",
        refused_as: "gate-run-script",
    },
    WorkflowEscape {
        name: "MUT-MSRV-RUN-RETARGETED",
        escape: "the MSRV leg checks out `master` ahead of its pinned command; the floor is \
                 verified on a tree the candidate never touched",
        job: Some("msrv"),
        anchor: "      - run: cargo check --locked --all-targets --all-features\n",
        replacement: "      - run: git fetch origin master && git checkout --detach FETCH_HEAD\n\
                      \x20     - run: cargo check --locked --all-targets --all-features\n",
        refused_as: "msrv-run-script",
    },
    WorkflowEscape {
        name: "MUT-STEP-USES-UNPINNED",
        escape: "the self-hosted job's checkout floats to `actions/checkout@v4`: the same \
                 action at whatever commit the tag points to, which is code nobody here \
                 reviewed with a checkout of its own",
        job: Some("test-windows"),
        anchor: "      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0\n",
        replacement: "      - uses: actions/checkout@v4\n",
        refused_as: "unpinned-action",
    },
    WorkflowEscape {
        name: "MUT-GATE-TOOLCHAIN-DOWNGRADED",
        escape: "the Windows gate installs the guest's own `1.97.1` instead of `stable`: the \
                 hosted witness then links the same toolchain the self-hosted leg links, so no \
                 leg compiles the Windows tree on current stable and a build script emitting a \
                 bad link directive only on newer rustc goes green",
        job: Some("lint-windows"),
        anchor: "          toolchain: stable\n",
        replacement: "          toolchain: 1.97.1\n",
        refused_as: "gate-toolchain",
    },
    WorkflowEscape {
        name: "MUT-DEFAULTS-WORKING-DIRECTORY",
        escape: "a job-level default working directory points every `run:` step at another crate: \
                 the pinned test command tests an empty `ci-pass/` while the labels, shells, \
                 checkout and command all still match character for character",
        job: Some("test-windows"),
        anchor: "\x20   timeout-minutes: ${{ matrix.shard != 'all' && 55 || 20 }}\n",
        replacement: "\x20   timeout-minutes: ${{ matrix.shard != 'all' && 55 || 20 }}\n\
                      \x20   defaults:\n\
                      \x20     run:\n\
                      \x20       working-directory: ci-pass\n",
        refused_as: "defaults-shape",
    },
    WorkflowEscape {
        name: "MUT-CACHE-CMD-FORMAT",
        escape: "the cache action, allowlisted and pinned by commit, is given a `cmd-format` \
                 input that wraps every command it runs in a `git checkout` of `master`. The \
                 checkout step is still input-free, every `run:` is still exact, and Clippy and \
                 the build witness examine a tree the candidate never touched.",
        job: Some("lint-windows"),
        anchor: "      - uses: Swatinem/rust-cache@6323deb102c322ba6fcbdcafc7e3dddab59af2b6 # v2.9.2\n",
        replacement: "      - uses: Swatinem/rust-cache@6323deb102c322ba6fcbdcafc7e3dddab59af2b6 # v2.9.2\n\
                      \x20       with:\n\
                      \x20         cmd-format: 'cmd /d /s /c \"git fetch origin master && git checkout --detach FETCH_HEAD && {0}\"'\n",
        refused_as: "unpinned-action-input",
    },
    WorkflowEscape {
        name: "MUT-COMPONENTS-INJECTED",
        escape: "the toolchain action's `components` value carries shell: the action builds a \
                 command from it and interpolates it into a Bash line, so `clippy` plus a \
                 checkout of `master` installs Clippy, moves the tree, and exits zero -- \
                 through an allowlisted action, at a pinned commit, with an allowlisted input \
                 name",
        job: Some("lint-windows"),
        anchor: "          components: clippy\n",
        replacement: "          components: 'clippy;git${IFS}fetch${IFS}origin${IFS}master&&git${IFS}checkout${IFS}--detach${IFS}FETCH_HEAD;true'\n",
        refused_as: "unpinned-action-input",
    },
    WorkflowEscape {
        name: "MUT-TOOLCHAIN-NOT-FIRST",
        escape: "a Cargo command is placed above the toolchain install, so it runs on whatever \
                 the runner image preinstalled rather than on the toolchain this workflow \
                 names. During a release rollout that is the previous stable, and the \
                 current-stable witness on this job would be compiling something else with \
                 every other pin still matching.",
        job: Some("lint-windows"),
        anchor: "      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0\n",
        replacement: "      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0\n\
                      \x20     - run: cargo fmt --check\n",
        refused_as: "gate-toolchain",
    },
    WorkflowEscape {
        name: "MUT-WORKFLOW-PERMISSIONS-WIDENED",
        escape: "the job token widened to `write-all`: every build script and test the \
                 candidate ships runs with a token that can push, the checkout leaves that \
                 credential configured for later Git commands, and destroying the guest \
                 afterwards recalls nothing",
        job: None,
        anchor: "permissions:\n  contents: read\n",
        replacement: "permissions: write-all\n",
        refused_as: "workflow-permissions",
    },
    WorkflowEscape {
        name: "MUT-WORKFLOW-ENV-TARGET-RUNNER",
        escape: "a Cargo target runner at workflow scope: every Windows test harness compiles \
                 and none executes, because Cargo hands each one to `cmd /c echo`, which prints \
                 the path it was given and exits zero. No Unix leg reads a Windows-target \
                 variable, and the command, shell, labels and checkout all still match.",
        job: None,
        anchor: "  RUSTFLAGS: -D warnings\n",
        replacement: "  RUSTFLAGS: -D warnings\n\
                      \x20 CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUNNER: cmd /c echo\n",
        refused_as: "workflow-env",
    },
    WorkflowEscape {
        name: "MUT-WITNESS-CHECKOUT-REF",
        escape: "the witness carrier's checkout points at `master`: the current-stable build \
                 witness links a tree the candidate never touched while the guest, one stable \
                 behind, links and passes the candidate",
        job: Some("lint-windows"),
        anchor: "      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0\n",
        replacement: "      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0\n\
                      \x20       with:\n\
                      \x20         ref: master\n",
        refused_as: "gate-checkout",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-CHECKOUT-REF",
        escape: "the Windows job's checkout points at `master`. Every other leg still reads \
                 the candidate; this job, on either lane, tests a tree the candidate never \
                 touched, and a Windows-only regression goes green.",
        job: Some("test-windows"),
        anchor: "      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0\n",
        replacement: "      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0\n\
                      \x20       with:\n\
                      \x20         ref: master\n",
        refused_as: "test-windows-checkout",
    },
    WorkflowEscape {
        name: "MUT-TEST-CHECKOUT-REF",
        escape: "the hosted test matrix checks out `master`: two platforms test a tree the \
                 candidate never touched while Clippy and MSRV read the candidate",
        job: Some("test"),
        anchor: "      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0\n",
        replacement: "      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0\n\
                      \x20       with:\n\
                      \x20         ref: master\n",
        refused_as: "test-job-checkout",
    },
    WorkflowEscape {
        name: "MUT-WINDOWS-BUILD-WITNESS-DELETED",
        escape: "the hosted `cargo build --all-targets` step removed. Clippy and MSRV still \
                 type-check the Windows tree on GitHub's runner, but nothing on current \
                 stable code-generates or links it: a Windows-only codegen or link failure \
                 passes every hosted leg while the guest, one stable behind, links and passes.",
        job: Some("lint-windows"),
        anchor: "      - run: cargo build --all-targets --all-features\n",
        replacement: "",
        refused_as: "windows-build-witness",
    },
    WorkflowEscape {
        name: "MUT-WINDOWS-BUILD-WITNESS-NARROWED",
        escape: "the witness narrowed to `cargo test --no-run`, which builds only test-profile \
                 artifacts: a `#[cfg(all(windows, not(test)))]` body in a binary is never \
                 linked on current stable, and a link failure there passes every hosted leg",
        job: Some("lint-windows"),
        anchor: "      - run: cargo build --all-targets --all-features\n",
        replacement: "      - run: cargo test --no-run --all-targets --all-features\n",
        refused_as: "windows-build-witness",
    },
    WorkflowEscape {
        name: "MUT-RUSTFLAGS-WEAKENED",
        escape: "warnings allowed instead of denied, at workflow scope. Every leg still runs \
                 every command, every job is green, and `unfulfilled_lint_expectations` -- \
                 warn-by-default -- stops retiring the `#[expect]`s that `CODING_STANDARDS.md` \
                 §11 says self-retire, so a suppression that no longer suppresses anything \
                 reads as enforcement forever.",
        job: None,
        anchor: "  RUSTFLAGS: -D warnings\n",
        replacement: "  RUSTFLAGS: -A warnings\n",
        refused_as: "rustflags",
    },
    WorkflowEscape {
        name: "MUT-RUSTFLAGS-ALLOW-APPENDED",
        escape: "an allow appended after the deny. The line still *contains* `-D warnings`, so \
                 every substring reading of it passes, while the effect denylist this whole \
                 file exists to enforce is switched off on every leg at once. The exact reason \
                 the pin is an equality.",
        job: None,
        anchor: "  RUSTFLAGS: -D warnings\n",
        replacement: "  RUSTFLAGS: -D warnings -A clippy::disallowed_methods\n",
        refused_as: "rustflags",
    },
    WorkflowEscape {
        name: "MUT-RUSTFLAGS-DELETED",
        escape: "the binding is gone and the `env:` block it lived in stays, so the workflow \
                 still declares the field this contract's top-level equality requires",
        job: None,
        anchor: "  RUSTFLAGS: -D warnings\n",
        replacement: "",
        refused_as: "rustflags",
    },
    WorkflowEscape {
        name: "MUT-RUSTFLAGS-VALUE-EMPTIED",
        escape: "the key stays and the value goes. YAML reads the empty value as null, so a \
                 reader that asks whether the name is bound gets yes and a reader that asks \
                 what it is bound to gets nothing.",
        job: None,
        anchor: "  RUSTFLAGS: -D warnings\n",
        replacement: "  RUSTFLAGS:\n",
        refused_as: "rustflags",
    },
    WorkflowEscape {
        name: "MUT-RUSTFLAGS-ENCODED-AT-WORKFLOW-SCOPE",
        escape: "`CARGO_ENCODED_RUSTFLAGS` beside the pinned line. Cargo reads it in \
                 preference to `RUSTFLAGS` and ignores `RUSTFLAGS` entirely when it is set, so \
                 the pin below it is read past rather than obeyed -- and it is still there, \
                 character for character, for any equality that reads only that line.",
        job: None,
        anchor: "  RUSTFLAGS: -D warnings\n",
        replacement: "  RUSTFLAGS: -D warnings\n  CARGO_ENCODED_RUSTFLAGS: ''\n",
        refused_as: "rustflags",
    },
    WorkflowEscape {
        name: "MUT-RUSTFLAGS-JOB-OVERRIDE",
        escape: "the narrowing `CODING_STANDARDS.md` §11 names, one job at a time: a job-level \
                 `env:` shadows the workflow-scope value for every step it covers while the \
                 pinned line stays untouched at the top of the file. The `msrv` leg is the \
                 mutation site because until this change it had no field set to refuse an \
                 `env:` at all.",
        job: Some("msrv"),
        anchor: "    timeout-minutes: 15\n",
        replacement: "    timeout-minutes: 15\n\
                      \x20   env:\n\
                      \x20     RUSTFLAGS: -A warnings\n",
        refused_as: "rustflags-override",
    },
    WorkflowEscape {
        name: "MUT-RUSTFLAGS-ENCODED-JOB-OVERRIDE",
        escape: "the same narrowing under the name Cargo prefers, so the job's `RUSTFLAGS` is \
                 not rewritten but ignored. Nothing in this workflow mentions `RUSTFLAGS` in \
                 that job for a reader to notice.",
        job: Some("test"),
        anchor: "    timeout-minutes: 30\n",
        replacement: "    timeout-minutes: 30\n\
                      \x20   env:\n\
                      \x20     CARGO_ENCODED_RUSTFLAGS: ''\n",
        refused_as: "rustflags-override",
    },
    WorkflowEscape {
        name: "MUT-RUSTFLAGS-STEP-OVERRIDE",
        escape: "the narrowing one level down, in the aggregate's step, whose field set allows \
                 an `env:`. A step-level binding is the smallest form of the \
                 same defect and the one a field-set equality cannot see, because the field is \
                 legal there.",
        job: Some("merge-gate"),
        anchor: "          LINT_RESULT: ${{ needs.lint.result }}\n",
        replacement: "          LINT_RESULT: ${{ needs.lint.result }}\n\
                      \x20         RUSTFLAGS: -A warnings\n",
        refused_as: "rustflags-override",
    },
    WorkflowEscape {
        name: "MUT-MSRV-TOOLCHAIN-DRIFTED",
        escape: "the leg named for the floor installs `stable`. It runs, it passes, and it is \
                 evidence about whichever compiler the runner shipped that week rather than \
                 about the version `Cargo.toml` publishes to crates.io.",
        job: Some("msrv"),
        anchor: "          toolchain: 1.85.0\n",
        replacement: "          toolchain: stable\n",
        refused_as: "msrv-job-toolchain",
    },
    WorkflowEscape {
        name: "MUT-MSRV-TOOLCHAIN-AHEAD-OF-MANIFEST",
        escape: "a real version, above the declared floor. `test-docs-consistency.sh`'s C2 \
                 refuses this shape too, by grepping a text block. The difference is \
                 exactness, not coverage: C2 accepts `rust-version` \"or a patch release of \
                 it\", so it reads `toolchain: 1.85` as agreement, and that name resolves to \
                 the newest 1.85.x rather than to the `cargo +1.85.0` the documents publish.",
        job: Some("msrv"),
        anchor: "          toolchain: 1.85.0\n",
        replacement: "          toolchain: 1.90.0\n",
        refused_as: "msrv-job-toolchain",
    },
    WorkflowEscape {
        name: "MUT-MSRV-COMMAND-UNLOCKED",
        escape: "`--locked` dropped. Cargo re-resolves `Cargo.lock` forward past the exact \
                 pins this manifest carries for the floor -- `globset =0.4.19` and \
                 `yaml-rust2 =0.12.0` are both pinned against exactly this -- so the leg \
                 compiles a dependency set no release ships and reports green over a floor it \
                 never tested.",
        job: Some("msrv"),
        anchor: "      - run: cargo check --locked --all-targets --all-features\n",
        replacement: "      - run: cargo check --all-targets --all-features\n",
        refused_as: "msrv-job-command",
    },
    WorkflowEscape {
        name: "MUT-MSRV-COMMAND-ECHOED",
        escape: "the leg echoes its command and succeeds, `MUT-GATE-ECHOED`'s shape one job \
                 over. `test-docs-consistency.sh` withdrew every claim about which commands CI \
                 runs precisely because a text checker cannot tell these apart.",
        job: Some("msrv"),
        anchor: "      - run: cargo check --locked --all-targets --all-features\n",
        replacement: "      - run: echo cargo check --locked --all-targets --all-features\n",
        refused_as: "msrv-job-command",
    },
    WorkflowEscape {
        name: "MUT-MSRV-MATRIX-NARROWED",
        escape: "the floor is checked on Linux only. A dependency that raises its MSRV behind \
                 a `cfg` -- this manifest has a `cfg(windows)` and a `cfg(unix)` dependency \
                 table -- breaks on a platform this leg no longer visits.",
        job: Some("msrv"),
        anchor: "        os: [ubuntu-latest, windows-latest, macos-latest]\n",
        replacement: "        os: [ubuntu-latest]\n",
        refused_as: "msrv-job-matrix",
    },
    WorkflowEscape {
        name: "MUT-MSRV-MATRIX-EXCLUDED",
        escape: "the same narrowing with `os:` left listing all three, so every reading of \
                 `os:` passes while the Windows floor is never compiled. \
                 `MUT-TEST-MATRIX-EXCLUDED`'s shape on the leg that had no strategy contract.",
        job: Some("msrv"),
        anchor: "        os: [ubuntu-latest, windows-latest, macos-latest]\n",
        replacement: "        os: [ubuntu-latest, windows-latest, macos-latest]\n\
                      \x20       exclude:\n\
                      \x20         - os: windows-latest\n",
        refused_as: "msrv-job-matrix",
    },
    WorkflowEscape {
        name: "MUT-MSRV-FAIL-FAST-ENABLED",
        escape: "one platform's floor failure cancels the other two before they report, so a \
                 break on two platforms is indistinguishable from a break on one and the \
                 second is invisible until the first is fixed",
        job: Some("msrv"),
        anchor: "      fail-fast: false\n",
        replacement: "      fail-fast: true\n",
        refused_as: "msrv-job-matrix",
    },
    WorkflowEscape {
        name: "MUT-MSRV-JOB-ABSOLVED",
        escape: "`continue-on-error` on the leg. This is the one the aggregate cannot catch: a \
                 *skipped* job still reports `skipped` and the aggregate's loop demands \
                 `success`, but an absolved job reports `success` after its check failed, so \
                 `upstroke-ci` settles green over an unmet floor on all three platforms.",
        job: Some("msrv"),
        anchor: "    runs-on: ${{ matrix.os }}\n",
        replacement: "    runs-on: ${{ matrix.os }}\n    continue-on-error: true\n",
        refused_as: "msrv-job-field",
    },
    WorkflowEscape {
        name: "MUT-MSRV-STEP-ABSOLVED",
        escape: "the same absolution one level down, on the step that runs the check: the step \
                 fails, the job succeeds, and the leg is green",
        job: Some("msrv"),
        anchor: "      - run: cargo check --locked --all-targets --all-features\n",
        replacement: "      - run: cargo check --locked --all-targets --all-features\n\
                      \x20       continue-on-error: true\n",
        refused_as: "msrv-job-step-field",
    },
    WorkflowEscape {
        name: "MUT-RUSTFLAGS-PERSISTED-VIA-GITHUB-ENV",
        escape: "the narrowing with no `env:` mapping anywhere in the document. One `run:` \
                 line writes the job-scoped environment file, and every step after it in the \
                 job -- the `cargo check` on the next line -- compiles under `-A warnings`. \
                 Every field-set equality passes, the pinned workflow line is untouched, and \
                 the whole defect lives inside a scalar this contract used to treat as \
                 opaque.",
        job: Some("msrv"),
        anchor: "      - run: cargo check --locked --all-targets --all-features\n",
        replacement: "      - run: echo \"RUSTFLAGS=-A warnings\" >> \"$GITHUB_ENV\"\n\
                      \x20     - run: cargo check --locked --all-targets --all-features\n",
        refused_as: "rustflags-persisted",
    },
    WorkflowEscape {
        name: "MUT-RUSTFLAGS-PERSISTED-VIA-EXPRESSION",
        escape: "the same write through `${{ github.env }}`, the expression form of the same \
                 file, and under the name Cargo prefers. Nothing in the line spells \
                 `GITHUB_ENV`, so a reading that looks for that one token misses it entirely.",
        job: Some("lint"),
        anchor: "      - run: cargo fmt --check\n",
        replacement: "      - run: echo \"CARGO_ENCODED_RUSTFLAGS=\" >> ${{ github.env }}\n\
                      \x20     - run: cargo fmt --check\n",
        refused_as: "rustflags-persisted",
    },
    WorkflowEscape {
        name: "MUT-RUSTFLAGS-PERSISTED-VIA-POWERSHELL",
        escape: "the same write in PowerShell, on the leg whose default shell is `pwsh`. \
                 `Add-Content -Path $env:GITHUB_ENV` is how the file is reached there, and it \
                 shares no syntax at all with the bash form -- which is the point of covering \
                 the shells the runners actually resolve to.",
        job: Some("lint-windows"),
        anchor: "      - run: cargo clippy --all-targets --all-features -- -D warnings\n",
        replacement: "      - run: Add-Content -Path $env:GITHUB_ENV -Value \"RUSTFLAGS=-A warnings\"\n\
                      \x20     - run: cargo clippy --all-targets --all-features -- -D warnings\n",
        refused_as: "rustflags-persisted",
    },
    WorkflowEscape {
        name: "MUT-RUSTFLAGS-PERSISTED-VIA-HEREDOC",
        escape: "the same write with the name and the redirection on different lines. A \
                 heredoc is the form that defeats any reading anchored to `echo` or to a \
                 single line, and the job it lands in is the one that runs these fixtures.",
        job: Some("test"),
        anchor: "      - run: cargo test --all-targets --all-features\n",
        replacement: "      - run: |\n\
                      \x20         cat >> \"$GITHUB_ENV\" <<'EOF'\n\
                      \x20         RUSTFLAGS=-A warnings\n\
                      \x20         EOF\n\
                      \x20     - run: cargo test --all-targets --all-features\n",
        refused_as: "rustflags-persisted",
    },
    WorkflowEscape {
        name: "MUT-RUSTFLAGS-COMMAND-SCOPED",
        escape: "the narrowing without the env file at all: the flags are scoped to one \
                 command on one line. It persists nothing, so it is the arm that proves the \
                 script scan is not merely a `GITHUB_ENV` detector -- and a leg that names the \
                 variable at all is doing something this contract has no model for.",
        job: Some("lint-macos"),
        anchor: "      - run: cargo clippy --all-targets --all-features -- -D warnings\n",
        replacement: "      - run: RUSTFLAGS=-A warnings cargo build --all-targets\n\
                      \x20     - run: cargo clippy --all-targets --all-features -- -D warnings\n",
        refused_as: "rustflags-in-script",
    },
    WorkflowEscape {
        name: "MUT-RUSTFLAGS-JOB-OVERRIDE-LOWERCASE",
        escape: "the job-level narrowing spelled in lower case. On the two Unix legs it binds \
                 a different variable and does nothing; on `windows-latest` the process \
                 environment is case-insensitive and it *is* `RUSTFLAGS`. A case-sensitive \
                 reading refuses it on no platform, which is the Linux-only half of a \
                 platform-shaped mutation.",
        job: Some("msrv"),
        anchor: "    timeout-minutes: 15\n",
        replacement: "    timeout-minutes: 15\n\
                      \x20   env:\n\
                      \x20     rustflags: -A warnings\n",
        refused_as: "rustflags-override",
    },
    WorkflowEscape {
        name: "MUT-RUSTFLAGS-STEP-OVERRIDE-MIXED-CASE",
        escape: "the same in mixed case, one level down, in the aggregate's step",
        job: Some("merge-gate"),
        anchor: "          LINT_RESULT: ${{ needs.lint.result }}\n",
        replacement: "          LINT_RESULT: ${{ needs.lint.result }}\n\
                      \x20         RustFlags: -A warnings\n",
        refused_as: "rustflags-override",
    },
    WorkflowEscape {
        name: "MUT-RUSTFLAGS-WORKFLOW-CASE-VARIANT",
        escape: "a case variant beside the pinned line at workflow scope. The pin still reads \
                 back character for character, and on `windows-latest` the two keys are one \
                 variable whose winner this document does not decide.",
        job: None,
        anchor: "  RUSTFLAGS: -D warnings\n",
        replacement: "  RUSTFLAGS: -D warnings\n  Rustflags: -A warnings\n",
        refused_as: "rustflags",
    },
    WorkflowEscape {
        name: "MUT-RUSTFLAGS-ENCODED-LOWERCASE",
        escape: "the encoded name in lower case. `MUT-RUSTFLAGS-ENCODED-AT-WORKFLOW-SCOPE` \
                 covers the upper-case form; this is the one an exact-key reading would let \
                 through on the platform where it works.",
        job: None,
        anchor: "  RUSTFLAGS: -D warnings\n",
        replacement: "  RUSTFLAGS: -D warnings\n  cargo_encoded_rustflags: ''\n",
        refused_as: "rustflags",
    },
    WorkflowEscape {
        name: "MUT-MSRV-CHECK-BEFORE-TOOLCHAIN",
        escape: "both steps present, both exact, and the check above the install. \
                 `dtolnay/rust-toolchain` selects the toolchain for the steps that follow it, \
                 so the check compiles on whatever the runner image preinstalled -- stable -- \
                 and the leg is green about a version this crate publishes no floor for. \
                 Nothing in the presence-and-equality contract could see it: the step count, \
                 the command, the matrix and the toolchain scalar are all unchanged.",
        job: Some("msrv"),
        anchor: "      - uses: dtolnay/rust-toolchain@4360b52568e2003a75bf9bc1d59f33a8e3fc893c # stable\n\
                 \x20       with:\n\
                 \x20         toolchain: 1.85.0\n\
                 \x20     - uses: Swatinem/rust-cache@6323deb102c322ba6fcbdcafc7e3dddab59af2b6 # v2.9.2\n\
                 \x20     - run: cargo check --locked --all-targets --all-features\n",
        replacement: "      - run: cargo check --locked --all-targets --all-features\n\
                      \x20     - uses: dtolnay/rust-toolchain@4360b52568e2003a75bf9bc1d59f33a8e3fc893c # stable\n\
                      \x20       with:\n\
                      \x20         toolchain: 1.85.0\n\
                      \x20     - uses: Swatinem/rust-cache@6323deb102c322ba6fcbdcafc7e3dddab59af2b6 # v2.9.2\n",
        refused_as: "msrv-job-order",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-SHARD-DROPPED",
        escape: "the merge-queue lane runs three of the four Windows shards. Each leg that runs \
                 is green and every pin on its steps holds; the fourth shard's tests run nowhere \
                 on the lane that lands the merge.",
        job: Some("test-windows"),
        anchor: "&& '[\"w1\", \"w2\", \"w3\", \"w4\"]' ||",
        replacement: "&& '[\"w1\", \"w2\", \"w3\"]' ||",
        refused_as: "test-windows-matrix",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-SHARD-ADDED",
        escape: "a fifth Windows shard the table does not hold. Its leg finds no filters for \
                 itself, and the verifier refuses a record from a shard its table does not name; \
                 the contract refuses the drift before either runs.",
        job: Some("test-windows"),
        anchor: "&& '[\"w1\", \"w2\", \"w3\", \"w4\"]' ||",
        replacement: "&& '[\"w1\", \"w2\", \"w3\", \"w4\", \"w5\"]' ||",
        refused_as: "test-windows-matrix",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-QUEUE-UNSHARDED",
        escape: "the merge-queue lane back on one whole-suite leg. With the shard-keyed routing \
                 that leg is the guest's, so the queue waits on the build box and its pull \
                 requests -- the shape the hosted queue lane was made to retire, and on the \
                 hosted side the whole suite is the leg the timeouts this partition exists for \
                 came from.",
        job: Some("test-windows"),
        anchor: "&& '[\"w1\", \"w2\", \"w3\", \"w4\"]' ||",
        replacement: "&& '[\"all\"]' ||",
        refused_as: "test-windows-matrix",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-PR-LANE-WITHOUT-GUEST",
        escape: "the pull-request lane loses the guest's whole suite, the one Windows suite that \
                 lane has, for a hosted shard: every pull request waits on a hosted Windows leg \
                 and runs a part of the suite there.",
        job: Some("test-windows"),
        anchor: "|| '[\"all",
        replacement: "|| '[\"w1",
        refused_as: "test-windows-matrix",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-FAIL-FAST",
        escape: "one Windows shard's failure cancels the other three before they report, so a run \
                 with two broken shards shows one",
        job: Some("test-windows"),
        anchor: "\x20   strategy:\n\
                 \x20     fail-fast: false\n",
        replacement: "\x20   strategy:\n\
                      \x20     fail-fast: true\n",
        refused_as: "test-windows-matrix",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-GUEST-RENAMED",
        escape: "the guest's leg renamed. Every check still passes, and the scheduling guards \
                 that look for `test (winguest)` and its three labels find nothing to hold back.",
        job: Some("test-windows"),
        anchor: "\x20   name: test (${{ matrix.shard == 'all' && 'winguest' || format('windows-latest, {0}', matrix.shard) }})\n",
        replacement: "\x20   name: test (${{ matrix.shard == 'all' && 'windows-guest' || format('windows-latest, {0}', matrix.shard) }})\n",
        refused_as: "test-windows-name",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-SHARD-NAMES-COLLAPSE",
        escape: "the shard dropped from the hosted legs' name: four legs publish one check name, \
                 and nothing reading the run can tell which shard passed or timed out",
        job: Some("test-windows"),
        anchor: "\x20   name: test (${{ matrix.shard == 'all' && 'winguest' || format('windows-latest, {0}', matrix.shard) }})\n",
        replacement: "\x20   name: test (${{ matrix.shard == 'all' && 'winguest' || 'windows-latest' }})\n",
        refused_as: "test-windows-name",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-CEILINGS-SWAPPED",
        escape: "the two ceilings swapped: the guest's whole suite gets the hosted lane's 55 \
                 minutes and every hosted shard the guest's 20",
        job: Some("test-windows"),
        anchor: "\x20   timeout-minutes: ${{ matrix.shard != 'all' && 55 || 20 }}\n",
        replacement: "\x20   timeout-minutes: ${{ matrix.shard != 'all' && 20 || 55 }}\n",
        refused_as: "test-windows-timeout",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-SHARD-STEP-DELETED",
        escape: "the shard step gone: each hosted leg checks out, installs, uploads nothing and \
                 fails at the upload -- or, with the upload gone too, reports success having run \
                 nothing",
        job: Some("test-windows"),
        anchor: "\x20     - name: Test one shard, and record what it listed, selected and ran\n\
                 \x20       if: matrix.shard != 'all'\n\
                 \x20       env:\n\
                 \x20         UPSTROKE_TEST_TEMP_FOLDS_CASE: \"1\"\n\
                 \x20       run: |\n\
                 \x20         $rustc = ((cargo rustc -q --lib -- --version) | Out-String).Trim(); $path_rustc = ((rustc --version) | Out-String).Trim(); $cargo = ((cargo --version) | Out-String).Trim()\n\
                 \x20         if ($rustc -notmatch '^rustc 1\\.97\\.1 ' -or $path_rustc -notmatch '^rustc 1\\.97\\.1 ' -or $cargo -notmatch '^cargo 1\\.97\\.1 ') { throw \"the suite would run on '$rustc' (Cargo's compiler), '$path_rustc' (PATH's rustc) and '$cargo', not the image's 1.97.1: a compiler was chosen by something other than this workflow\" }\n\
                 \x20         python .github/scripts/partition.py run windows ${{ matrix.shard }} test-partition\n\
                 \x20         if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }\n",
        replacement: "",
        refused_as: "test-windows-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-SHARD-ON-EVERY-LEG",
        escape: "the shard step's condition dropped: the guest's leg runs its whole suite and \
                 then a shard named `all`, which the table does not hold, so the pull-request \
                 lane fails on a step no pull request needs",
        job: Some("test-windows"),
        anchor: "\x20     - name: Test one shard, and record what it listed, selected and ran\n\
                 \x20       if: matrix.shard != 'all'\n",
        replacement: "\x20     - name: Test one shard, and record what it listed, selected and ran\n",
        refused_as: "test-windows-suite-lane",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-SHARD-WITHOUT-COMPILER-CHECK",
        escape: "the shard step loses the compiler check: the hosted shards run on whatever Cargo \
                 resolves to while the guest's leg still refuses every compiler but the image's",
        job: Some("test-windows"),
        anchor: "\x20         $rustc = ((cargo rustc -q --lib -- --version) | Out-String).Trim(); $path_rustc = ((rustc --version) | Out-String).Trim(); $cargo = ((cargo --version) | Out-String).Trim()\n\
                 \x20         if ($rustc -notmatch '^rustc 1\\.97\\.1 ' -or $path_rustc -notmatch '^rustc 1\\.97\\.1 ' -or $cargo -notmatch '^cargo 1\\.97\\.1 ') { throw \"the suite would run on '$rustc' (Cargo's compiler), '$path_rustc' (PATH's rustc) and '$cargo', not the image's 1.97.1: a compiler was chosen by something other than this workflow\" }\n\
                 \x20         python .github/scripts/partition.py run windows ${{ matrix.shard }} test-partition\n",
        replacement: "\x20         python .github/scripts/partition.py run windows ${{ matrix.shard }} test-partition\n",
        refused_as: "test-windows-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-SHARD-UNFILTERED",
        escape: "the shard step runs the whole suite on every hosted leg: each shard is green, \
                 the record is never written, and four legs each pay for the whole suite under \
                 the ceiling one whole-suite leg already ran into",
        job: Some("test-windows"),
        anchor: "\x20         python .github/scripts/partition.py run windows ${{ matrix.shard }} test-partition\n",
        replacement: "\x20         cargo test --all-targets --all-features\n",
        refused_as: "test-windows-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-UPLOAD-DROPPED",
        escape: "the record is never uploaded: every hosted shard is green and the verifier has \
                 nothing to read -- it refuses, but only because it fails closed; the contract \
                 refuses the shape itself",
        job: Some("test-windows"),
        anchor: "\x20     - name: Upload the shard's record\n\
                 \x20       if: matrix.shard != 'all'\n\
                 \x20       uses: actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a # v7.0.1\n\
                 \x20       with:\n\
                 \x20         name: test-partition-${{ matrix.shard }}-attempt-${{ github.run_attempt }}\n\
                 \x20         path: test-partition\n\
                 \x20         if-no-files-found: error\n",
        replacement: "",
        refused_as: "test-windows-upload",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-UPLOAD-WITHOUT-ATTEMPT",
        escape: "the record's name drops the run attempt: a re-run of a shard collides with the \
                 record of the attempt it replaces, and the verifier can no longer tell the \
                 superseded record from the current one",
        job: Some("test-windows"),
        anchor: "\x20         name: test-partition-${{ matrix.shard }}-attempt-${{ github.run_attempt }}\n",
        replacement: "\x20         name: test-partition-${{ matrix.shard }}\n",
        refused_as: "test-windows-upload",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-UPLOAD-ON-FAILURE",
        escape: "the upload runs after a failed shard too, so a record of a run that did not \
                 finish is uploaded beside a red leg",
        job: Some("test-windows"),
        anchor: "\x20     - name: Upload the shard's record\n\
                 \x20       if: matrix.shard != 'all'\n\
                 \x20       uses: actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a # v7.0.1\n\
                 \x20       with:\n\
                 \x20         name: test-partition-${{ matrix.shard }}-attempt-${{ github.run_attempt }}\n\
                 \x20         path: test-partition\n\
                 \x20         if-no-files-found: error\n",
        replacement: "\x20     - name: Upload the shard's record\n\
                      \x20       if: always() && matrix.shard != 'all'\n\
                      \x20       uses: actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a # v7.0.1\n\
                      \x20       with:\n\
                      \x20         name: test-partition-${{ matrix.shard }}-attempt-${{ github.run_attempt }}\n\
                      \x20         path: test-partition\n\
                      \x20         if-no-files-found: error\n",
        refused_as: "test-windows-upload",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-UPLOAD-ABSOLVED",
        escape: "the upload is allowed to fail: the leg is green with no record, and only the \
                 verifier's missing-leg refusal stands between that and a lane with a shard \
                 unaccounted for",
        job: Some("test-windows"),
        anchor: "\x20     - name: Upload the shard's record\n\
                 \x20       if: matrix.shard != 'all'\n\
                 \x20       uses: actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a # v7.0.1\n\
                 \x20       with:\n\
                 \x20         name: test-partition-${{ matrix.shard }}-attempt-${{ github.run_attempt }}\n\
                 \x20         path: test-partition\n\
                 \x20         if-no-files-found: error\n",
        replacement: "\x20     - name: Upload the shard's record\n\
                      \x20       if: matrix.shard != 'all'\n\
                      \x20       uses: actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a # v7.0.1\n\
                      \x20       with:\n\
                      \x20         name: test-partition-${{ matrix.shard }}-attempt-${{ github.run_attempt }}\n\
                      \x20         path: test-partition\n\
                      \x20         if-no-files-found: error\n\
                      \x20       continue-on-error: true\n",
        refused_as: "unexpected-step-field",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-UPLOAD-UNPINNED",
        escape: "the upload action floats to a tag: code nobody here reviewed decides what \
                 reaches the verifier",
        job: Some("test-windows"),
        anchor: "\x20       uses: actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a # v7.0.1\n",
        replacement: "\x20       uses: actions/upload-artifact@v7\n",
        refused_as: "unpinned-action",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-UPLOAD-BEFORE-THE-SUITE",
        escape: "the upload moved above the shard step: it uploads nothing the shard ran, and the \
                 leg is green over a record that does not exist when it is taken",
        job: Some("test-windows"),
        anchor: "\x20     - name: Test one shard, and record what it listed, selected and ran\n\
                 \x20       if: matrix.shard != 'all'\n\
                 \x20       env:\n\
                 \x20         UPSTROKE_TEST_TEMP_FOLDS_CASE: \"1\"\n\
                 \x20       run: |\n\
                 \x20         $rustc = ((cargo rustc -q --lib -- --version) | Out-String).Trim(); $path_rustc = ((rustc --version) | Out-String).Trim(); $cargo = ((cargo --version) | Out-String).Trim()\n\
                 \x20         if ($rustc -notmatch '^rustc 1\\.97\\.1 ' -or $path_rustc -notmatch '^rustc 1\\.97\\.1 ' -or $cargo -notmatch '^cargo 1\\.97\\.1 ') { throw \"the suite would run on '$rustc' (Cargo's compiler), '$path_rustc' (PATH's rustc) and '$cargo', not the image's 1.97.1: a compiler was chosen by something other than this workflow\" }\n\
                 \x20         python .github/scripts/partition.py run windows ${{ matrix.shard }} test-partition\n\
                 \x20         if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }\n\n\
                 \x20     - name: Upload the shard's record\n\
                 \x20       if: matrix.shard != 'all'\n\
                 \x20       uses: actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a # v7.0.1\n\
                 \x20       with:\n\
                 \x20         name: test-partition-${{ matrix.shard }}-attempt-${{ github.run_attempt }}\n\
                 \x20         path: test-partition\n\
                 \x20         if-no-files-found: error\n",
        replacement: "\x20     - name: Upload the shard's record\n\
                      \x20       if: matrix.shard != 'all'\n\
                      \x20       uses: actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a # v7.0.1\n\
                      \x20       with:\n\
                      \x20         name: test-partition-${{ matrix.shard }}-attempt-${{ github.run_attempt }}\n\
                      \x20         path: test-partition\n\
                      \x20         if-no-files-found: error\n\n\
                      \x20     - name: Test one shard, and record what it listed, selected and ran\n\
                      \x20       if: matrix.shard != 'all'\n\
                      \x20       env:\n\
                      \x20         UPSTROKE_TEST_TEMP_FOLDS_CASE: \"1\"\n\
                      \x20       run: |\n\
                      \x20         $rustc = ((cargo rustc -q --lib -- --version) | Out-String).Trim(); $path_rustc = ((rustc --version) | Out-String).Trim(); $cargo = ((cargo --version) | Out-String).Trim()\n\
                      \x20         if ($rustc -notmatch '^rustc 1\\.97\\.1 ' -or $path_rustc -notmatch '^rustc 1\\.97\\.1 ' -or $cargo -notmatch '^cargo 1\\.97\\.1 ') { throw \"the suite would run on '$rustc' (Cargo's compiler), '$path_rustc' (PATH's rustc) and '$cargo', not the image's 1.97.1: a compiler was chosen by something other than this workflow\" }\n\
                      \x20         python .github/scripts/partition.py run windows ${{ matrix.shard }} test-partition\n\
                      \x20         if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }\n",
        refused_as: "test-windows-upload",
    },
    WorkflowEscape {
        name: "MUT-TEST-SHARD-STEP-DELETED",
        escape: "the macOS shard step gone: each macOS leg checks out, builds nothing and fails \
                 at the upload -- or, with the upload gone too, reports success having run \
                 nothing",
        job: Some("test"),
        anchor: "\x20     - name: Test one shard, and record what it listed, selected and ran\n\
                 \x20       if: matrix.shard != 'all'\n\
                 \x20       env:\n\
                 \x20         UPSTROKE_TEST_TEMP_FOLDS_CASE: ${{ matrix.os == 'macos-latest' && '1' || '' }}\n\
                 \x20       run: python3 .github/scripts/partition.py run macos ${{ matrix.shard }} test-partition\n",
        replacement: "",
        refused_as: "test-job-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-WHOLE-SUITE-ON-EVERY-LEG",
        escape: "the whole-suite step's condition dropped: every macOS shard leg runs the whole \
                 suite before its shard, which is the 30-minute leg this partition exists to \
                 split, three times",
        job: Some("test"),
        anchor: "\x20     - run: cargo test --all-targets --all-features\n\
                 \x20       if: matrix.shard == 'all'\n\
                 \x20       env:\n\
                 \x20         UPSTROKE_TEST_TEMP_FOLDS_CASE: ${{ matrix.os == 'macos-latest' && '1' || '' }}\n",
        replacement: "\x20     - run: cargo test --all-targets --all-features\n\
                      \x20       env:\n\
                      \x20         UPSTROKE_TEST_TEMP_FOLDS_CASE: ${{ matrix.os == 'macos-latest' && '1' || '' }}\n",
        refused_as: "test-suite-lane",
    },
    WorkflowEscape {
        name: "MUT-TEST-MACOS-SHARD-DROPPED",
        escape: "the macOS lane runs two of its three shards: each leg is green and the third \
                 shard's tests run nowhere",
        job: Some("test"),
        anchor: "\x20         - os: macos-latest\n\
                 \x20           shard: m3\n",
        replacement: "",
        refused_as: "test-job-matrix",
    },
    WorkflowEscape {
        name: "MUT-TEST-MACOS-SHARD-DUPLICATED",
        escape: "a shard listed twice and another not at all: the matrix still has four legs and \
                 the macOS group still three, but `m2`'s tests run twice and `m3`'s never",
        job: Some("test"),
        anchor: "\x20           shard: m3\n",
        replacement: "\x20           shard: m2\n",
        refused_as: "test-job-matrix",
    },
    WorkflowEscape {
        name: "MUT-TEST-UBUNTU-SHARDED",
        escape: "the ubuntu leg runs a shard instead of the whole suite: the Linux suite runs a \
                 third of itself and no verifier group covers the rest",
        job: Some("test"),
        anchor: "\x20         - os: ubuntu-latest\n\
                 \x20           shard: all\n",
        replacement: "\x20         - os: ubuntu-latest\n\
                      \x20           shard: m1\n",
        refused_as: "test-job-matrix",
    },
    WorkflowEscape {
        name: "MUT-TEST-NAME-WITHOUT-SHARD",
        escape: "the `test` job's name drops the shard: three macOS legs publish one check name",
        job: Some("test"),
        anchor: "\x20   name: test (${{ matrix.shard == 'all' && matrix.os || format('{0}, {1}', matrix.os, matrix.shard) }})\n",
        replacement: "\x20   name: test (${{ matrix.os }})\n",
        refused_as: "test-job-name",
    },
    WorkflowEscape {
        name: "MUT-TEST-UPLOAD-DROPPED",
        escape: "the macOS records are never uploaded: the shards are green and the verifier \
                 fails closed for want of them; the contract refuses the shape before that",
        job: Some("test"),
        anchor: "\x20     - name: Upload the shard's record\n\
                 \x20       if: matrix.shard != 'all'\n\
                 \x20       uses: actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a # v7.0.1\n\
                 \x20       with:\n\
                 \x20         name: test-partition-${{ matrix.shard }}-attempt-${{ github.run_attempt }}\n\
                 \x20         path: test-partition\n\
                 \x20         if-no-files-found: error\n",
        replacement: "",
        refused_as: "test-job-upload",
    },
    WorkflowEscape {
        name: "MUT-COVERAGE-JOB-DELETED",
        escape: "the coverage job gone, with the aggregate's wiring to it: every shard's green \
                 says only that its own selection passed, and nothing says the shards together \
                 ran the suite",
        job: None,
        anchor: "\n\
                 \x20 test-coverage:\n\
                 \x20   name: partition coverage\n\
                 \x20   needs: [test, test-windows]\n\
                 \x20   runs-on: ubuntu-latest\n\
                 \x20   timeout-minutes: 10\n\
                 \x20   steps:\n\
                 \x20     - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0\n\
                 \x20     - uses: actions/download-artifact@3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c # v8.0.1\n\
                 \x20       with:\n\
                 \x20         pattern: test-partition-*\n\
                 \x20         path: test-partition-records\n\
                 \x20     - run: python3 .github/scripts/partition.py verify test-partition-records\n",
        replacement: "\n",
        refused_as: "test-coverage-missing",
    },
    WorkflowEscape {
        name: "MUT-COVERAGE-RUNS-ALWAYS",
        escape: "the coverage job runs after a failed or cancelled leg: it reads the records the \
                 finished legs left and can report on a lane whose shards did not all finish",
        job: Some("test-coverage"),
        anchor: "\x20   runs-on: ubuntu-latest\n",
        replacement: "\x20   runs-on: ubuntu-latest\n\
                      \x20   if: always()\n",
        refused_as: "unexpected-job-field",
    },
    WorkflowEscape {
        name: "MUT-COVERAGE-ABSOLVED",
        escape: "the coverage job may fail and report success: a refusal settles green",
        job: Some("test-coverage"),
        anchor: "\x20   runs-on: ubuntu-latest\n",
        replacement: "\x20   runs-on: ubuntu-latest\n\
                      \x20   continue-on-error: true\n",
        refused_as: "unexpected-job-field",
    },
    WorkflowEscape {
        name: "MUT-COVERAGE-NEEDS-NARROWED",
        escape: "the coverage job waits for the macOS legs only: it can read the records before \
                 the Windows shards have uploaded theirs, and it is not skipped when one of them \
                 fails",
        job: Some("test-coverage"),
        anchor: "\x20   needs: [test, test-windows]\n",
        replacement: "\x20   needs: [test]\n",
        refused_as: "test-coverage-needs",
    },
    WorkflowEscape {
        name: "MUT-COVERAGE-OTHER-RUN",
        escape: "the download reads another run's artifacts: an earlier green run's records stand \
                 in for this run's legs",
        job: Some("test-coverage"),
        anchor: "\x20         path: test-partition-records\n",
        replacement: "\x20         path: test-partition-records\n\
                      \x20         run-id: 1\n\
                      \x20         github-token: ${{ github.token }}\n",
        refused_as: "test-coverage-download",
    },
    WorkflowEscape {
        name: "MUT-COVERAGE-DOWNLOAD-UNPINNED",
        escape: "the download action floats to a tag: code nobody here reviewed decides what the \
                 verifier reads",
        job: Some("test-coverage"),
        anchor: "\x20     - uses: actions/download-artifact@3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c # v8.0.1\n",
        replacement: "\x20     - uses: actions/download-artifact@v8\n",
        refused_as: "unpinned-action",
    },
    WorkflowEscape {
        name: "MUT-COVERAGE-VERIFIER-ECHOED",
        escape: "the verifier is echoed rather than run: the job succeeds and refuses nothing",
        job: Some("test-coverage"),
        anchor: "\x20     - run: python3 .github/scripts/partition.py verify test-partition-records\n",
        replacement: "\x20     - run: echo python3 .github/scripts/partition.py verify test-partition-records\n",
        refused_as: "test-coverage-steps",
    },
    WorkflowEscape {
        name: "MUT-COVERAGE-RUNS-A-DOWNLOADED-SCRIPT",
        escape: "the verifier is run from the downloaded records, so the code that judges the \
                 legs is code the legs uploaded",
        job: Some("test-coverage"),
        anchor: "\x20     - run: python3 .github/scripts/partition.py verify test-partition-records\n",
        replacement: "\x20     - run: python3 test-partition-records/partition.py verify test-partition-records\n",
        refused_as: "test-coverage-command",
    },
    WorkflowEscape {
        name: "MUT-COVERAGE-CHECKOUT-REF",
        escape: "the coverage job checks out `master`: the verifier and its table are master's, \
                 not the candidate's that the legs ran",
        job: Some("test-coverage"),
        anchor: "\x20     - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0\n",
        replacement: "\x20     - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0\n\
                      \x20       with:\n\
                      \x20         ref: master\n",
        refused_as: "test-coverage-checkout",
    },
    WorkflowEscape {
        name: "MUT-COVERAGE-STEP-BETWEEN",
        escape: "a step between the download and the verifier rewrites the records it then reads",
        job: Some("test-coverage"),
        anchor: "\x20     - run: python3 .github/scripts/partition.py verify test-partition-records\n",
        replacement: "\x20     - run: rm -rf test-partition-records\n\
                      \x20     - run: python3 .github/scripts/partition.py verify test-partition-records\n",
        refused_as: "test-coverage-steps",
    },
    WorkflowEscape {
        name: "MUT-AGGREGATE-COVERAGE-DROPPED",
        escape: "the aggregate does not wait for the coverage job: `upstroke-ci` settles green \
                 while the verifier refuses the lane",
        job: Some("merge-gate"),
        anchor: "\x20   needs: [lint, lint-windows, lint-macos, msrv, test, test-windows, test-coverage]\n",
        replacement: "\x20   needs: [lint, lint-windows, lint-macos, msrv, test, test-windows]\n",
        refused_as: "aggregate-needs",
    },
    WorkflowEscape {
        name: "MUT-AGGREGATE-COVERAGE-UNREAD",
        escape: "the aggregate waits for the coverage job and never reads its result: the loop \
                 names every other gate",
        job: Some("merge-gate"),
        anchor: "\x20         for gate in LINT LINT_WINDOWS LINT_MACOS MSRV TEST TEST_WINDOWS TEST_COVERAGE; do\n",
        replacement: "\x20         for gate in LINT LINT_WINDOWS LINT_MACOS MSRV TEST TEST_WINDOWS; do\n",
        refused_as: "aggregate-loop",
    },
];

pub(super) fn mutate_workflow(
    text: &str,
    job: Option<&str>,
    anchor: &str,
    replacement: &str,
) -> String {
    match job {
        Some(job) => mutate_job(text, job, anchor, replacement),
        None => {
            let hits = text.matches(anchor).count();
            assert_eq!(
                hits, 1,
                "the workflow contains {hits} copies of the mutation anchor, not one. It \
                 moved and this mutation measures nothing:\n{anchor}"
            );
            text.replacen(anchor, replacement, 1)
        }
    }
}

fn mutate_job(text: &str, job: &str, anchor: &str, replacement: &str) -> String {
    let jobs_at = text
        .find("\njobs:\n")
        .unwrap_or_else(|| panic!("{CI_WORKFLOW} has no `jobs:` mapping"));
    let header = format!("\n  {job}:\n");
    let start = jobs_at
        + text[jobs_at..]
            .find(&header)
            .unwrap_or_else(|| panic!("{CI_WORKFLOW} has no `{job}` job"))
        + 1;
    let rest = &text[start + header.len() - 1..];
    let end = rest
        .lines()
        .scan(0_usize, |offset, line| {
            let at = *offset;
            *offset += line.len() + 1;
            Some((at, line))
        })
        .find(|(at, line)| {
            *at > 0
                && line.len() > 2
                && line.starts_with("  ")
                && !line[2..].starts_with(' ')
                && line.ends_with(':')
                && line[2..line.len() - 1]
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        })
        .map_or(rest.len(), |(at, _)| at);
    let block = &rest[..end];
    let hits = block.matches(anchor).count();
    assert_eq!(
        hits, 1,
        "the `{job}` block contains {hits} copies of the mutation anchor, not one. The \
         workflow moved and this mutation measures nothing:\n{anchor}"
    );
    format!(
        "{}{}{}",
        &text[..start + header.len() - 1],
        block.replacen(anchor, replacement, 1),
        &rest[end..]
    )
}
