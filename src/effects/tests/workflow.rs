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
    AGGREGATE_STEP_FIELDS, CACHE_ACTION, CAPABILITY_OBSERVATION, CAPABILITY_STEP_FIELDS,
    CAPABILITY_STEP_TIMEOUT_MINUTES, CAPABILITY_WINDOWS_OBSERVATION, CI_TARGETS, CI_WORKFLOW,
    CLIPPY_GATE, CiTarget, DEFAULTS_FIELDS, DEFAULTS_RUN_FIELDS, ENCODED_RUSTFLAGS_KEY,
    GATE_JOB_FIELDS, GATE_SCRIPTS, GOLDEN_IMAGE_TOOLCHAIN, HOST_ACCEPTANCE_CAPTURE,
    HOST_ACCEPTANCE_WINDOWS_CAPTURE, KNOWN_SHELLS, MSRV_COMMAND, MSRV_ENTRY_KEYS, MSRV_JOB,
    MSRV_JOB_FIELDS, MSRV_MATRIX_FIELDS, MSRV_RUNS_ON, MSRV_TOOLCHAIN_TARGETS,
    OPTIONAL_DEFAULTS_FIELD, PINNED_ACTIONS, REQUIRED_CONTEXT, RETIRED_TEST_JOB, RUSTFLAGS_KEY,
    RUSTFLAGS_VALUE, STABLE_TOOLCHAIN, STEP_FIELDS, TEST_COMMAND, TEST_JOB_FIELDS, TEST_JOB_NAME,
    TEST_JOBS, TEST_SCRIPTS, TEST_SHARD_COMMAND, TEST_STEP_FIELDS, TEST_TIMEOUT_MINUTES,
    TEST_WINDOWS_HOSTED_JOB, TEST_WINDOWS_HOSTED_JOB_FIELDS, TEST_WINDOWS_HOSTED_NAME,
    TEST_WINDOWS_HOSTED_SCRIPTS, TEST_WINDOWS_HOSTED_TIMEOUT_MINUTES, TEST_WINDOWS_PLATFORM,
    TEST_WINDOWS_STEP_ENV, TOOLCHAIN_ACTION, TOOLCHAIN_COMPONENTS, TOOLCHAIN_TARGETS, TestJob,
    WINDOWS_BUILD_WITNESS, WINDOWS_SHARD_FILTERS, WINDOWS_SHARD_FLOORS, WINDOWS_SHARD_WITNESS,
    WORKFLOW_ENV, WORKFLOW_FIELDS, WORKFLOW_PERMISSIONS,
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

fn runner_target(runner: &str) -> Option<&'static CiTarget> {
    CI_TARGETS.iter().find(|target| target.runner == runner)
}

pub(super) fn cross_target(command: &str) -> Option<&str> {
    let mut words = command.split_whitespace();
    words.by_ref().find(|word| *word == "--target")?;
    words.next()
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

    let mut gate_jobs: BTreeMap<&str, &str> = BTreeMap::new();
    for target in &CI_TARGETS {
        let gates: Vec<&String> = job_names
            .iter()
            .filter(|name| {
                let Some(job) = field(jobs, name) else {
                    return false;
                };
                scalar(job, "runs-on") == Some(target.check_runner)
                    && steps_of(job)
                        .iter()
                        .any(|step| scalar(step, "run") == Some(target.clippy_gate))
            })
            .collect();
        if gates.len() != 1 {
            out.push(format!(
                "[no-gate-job] expected exactly one job whose `runs-on:` is `{}` and one of \
                 whose steps has `run:` equal to `{}`, found {gates:?}. Without it every body \
                 the `{}` tuple compiles is outside the denylist's reach on every job CI runs.",
                target.check_runner, target.clippy_gate, target.triple
            ));
            continue;
        }
        let gate = gates[0];
        if let Some(shared) = gate_jobs.insert(gate.as_str(), target.triple) {
            out.push(format!(
                "[no-gate-job] `{gate}` is the Clippy gate of both `{shared}` and `{}`. Each \
                 target's gate is a job of its own, so that one target's failure, timeout or \
                 cancellation cannot stand for another's.",
                target.triple
            ));
        }
        let Some(job) = field(jobs, gate) else {
            continue;
        };
        let Some(host) = runner_target(target.check_runner) else {
            out.push(format!(
                "[step-shell] `{gate}` runs on `{}`, which is not a runner this contract \
                 models, so the shell its steps resolve to is undecidable here",
                target.check_runner
            ));
            continue;
        };
        if let Some(triple) = cross_target(target.clippy_gate) {
            let pinned: Vec<Option<&str>> = steps_of(job)
                .iter()
                .filter(|step| installs_a_toolchain(step))
                .map(|step| field(step, "with").and_then(|with| scalar(with, "targets")))
                .collect();
            if pinned != [Some(triple)] {
                out.push(format!(
                    "[gate-toolchain-target] `{gate}` runs `{}` and its toolchain steps install \
                     the targets {pinned:?}, not exactly `{triple}`. The pin keeps the tuple \
                     the gate names in one place with the command that names it; without it \
                     the job fails for want of that tuple's standard library.",
                    target.clippy_gate
                ));
            }
        }

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
                    host,
                    &format!("`{gate}` step {index}"),
                    host.default_shell,
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
    let Some(jobs) = field(doc, "jobs") else {
        return vec!["[jobs] the workflow declares no `jobs:` mapping".to_owned()];
    };
    let mut out = test_jobs_table_complaints();
    if field(jobs, RETIRED_TEST_JOB).is_some() {
        out.push(format!(
            "[test-job-split] the workflow still declares a `{RETIRED_TEST_JOB}` job. The suite \
             runs on each platform that hosts it in a job of its own, {:?}, and the split is \
             the contract: a `{RETIRED_TEST_JOB}` job beside them runs legs no row of \
             `TEST_JOBS` holds to anything, and one in their place runs every platform's legs \
             under one `runs-on:` this contract no longer reads.",
            TEST_JOBS.iter().map(|row| row.id).collect::<Vec<&str>>()
        ));
    }

    for row in &TEST_JOBS {
        let id = row.id;
        let Some(job) = field(jobs, id) else {
            out.push(format!(
                "[test-job-missing] no `{id}` job, so nothing runs these fixtures on `{}`",
                row.runner
            ));
            continue;
        };

        for complaint in field_complaints(job, &TEST_JOB_FIELDS, &OPTIONAL_DEFAULTS_FIELD) {
            out.push(format!("[unexpected-job-field] `{id}` {complaint}"));
        }
        let host = runner_target(row.runner);
        if host.is_none() {
            out.push(format!(
                "[step-shell] `{id}` runs on `{}`, which is not a runner this contract models, \
                 so the shell its steps resolve to is undecidable here",
                row.runner
            ));
        }
        for (index, step) in steps_of(job).iter().enumerate() {
            let named = format!("`{id}` step {index}");
            let runs_the_suite = scalar(step, "run") == Some(TEST_SHARD_COMMAND);
            let runs_the_capture = scalar(step, "run") == Some(HOST_ACCEPTANCE_CAPTURE);
            let runs_the_observation = scalar(step, "run") == Some(CAPABILITY_OBSERVATION);
            let allowed: &[&str] = if runs_the_suite || runs_the_capture {
                &TEST_STEP_FIELDS
            } else if runs_the_observation {
                &CAPABILITY_STEP_FIELDS
            } else {
                &STEP_FIELDS
            };
            let strange = unexpected(&field_names(step), allowed);
            if !strange.is_empty() {
                out.push(format!(
                    "[unexpected-step-field] {named} declares {strange:?}"
                ));
            }
            if runs_the_suite {
                out.extend(step_env_complaints(
                    step,
                    &named,
                    "test-step-env",
                    &row.step_env,
                ));
            }
            if runs_the_capture {
                out.extend(step_env_complaints(
                    step,
                    &named,
                    "test-capture-step-env",
                    &row.step_env,
                ));
            }
            if runs_the_observation {
                out.extend(step_env_complaints(
                    step,
                    &named,
                    "test-capability-step-env",
                    &row.step_env,
                ));
                out.extend(observation_step_complaints(
                    step,
                    &named,
                    "test-capability-step",
                ));
            }
            if scalar(step, "run").is_some() {
                out.extend(
                    host.map(|target| {
                        shell_complaints(
                            doc,
                            job,
                            step,
                            target,
                            &format!("{named} on `{}`", target.runner),
                            target.default_shell,
                        )
                    })
                    .unwrap_or_default(),
                );
            }
        }

        let running = steps_of(job)
            .iter()
            .filter(|step| scalar(step, "run") == Some(TEST_SHARD_COMMAND))
            .count();
        if running != 1 {
            out.push(format!(
                "[test-job-command] `{id}` has {running} steps whose `run:` is exactly \
                 `{TEST_SHARD_COMMAND}`, not one. These fixtures are only evidence of anything \
                 in a job that executes them, and the line hands libtest the leg's \
                 `matrix.tests`: without it every leg of a divided suite runs the suite whole, \
                 under the {TEST_TIMEOUT_MINUTES} minutes set for a shard of it."
            ));
        }
        out.extend(capture_complaints(
            job,
            id,
            "test-capture-command",
            TEST_SHARD_COMMAND,
            HOST_ACCEPTANCE_CAPTURE,
        ));
        out.extend(observation_complaints(
            job,
            id,
            "test-capability-command",
            HOST_ACCEPTANCE_CAPTURE,
            CAPABILITY_OBSERVATION,
        ));
        out.extend(checkout_complaints(job, id, "test-job-checkout"));
        out.extend(step_pin_complaints(
            job,
            id,
            "test-job-run-script",
            &TEST_SCRIPTS,
        ));
        out.extend(toolchain_complaints(
            job,
            id,
            "test-job-toolchain",
            Some(STABLE_TOOLCHAIN),
        ));

        out.extend(test_leg_matrix_complaints(job, row));
        if scalar(job, "runs-on") != Some(row.runner) {
            out.push(format!(
                "[test-job-matrix] `{id}` runs on {:?}, not `{}`. The job runs the shards of \
                 `{}`'s suite: on another runner it runs that platform's shards elsewhere, and \
                 its legs are green over a platform none of them ran on. An expression or a \
                 label set is a runner this contract cannot read.",
                field(job, "runs-on"),
                row.runner,
                row.runner
            ));
        }
        if scalar(job, "name") != Some(TEST_JOB_NAME) {
            out.push(format!(
                "[test-job-name] `{id}` is named {:?}, not `{TEST_JOB_NAME}`. The name is how a \
                 run's list tells the legs apart: under a name that does not read `matrix.leg` \
                 the job's shards publish one name, so a red leg does not say which shard's \
                 tests failed.",
                scalar(job, "name")
            ));
        }
        let minutes = field(job, "timeout-minutes").and_then(Yaml::as_i64);
        if minutes != Some(TEST_TIMEOUT_MINUTES) {
            out.push(format!(
                "[test-job-timeout] `{id}` is bounded at {minutes:?} minutes, not exactly \
                 {TEST_TIMEOUT_MINUTES} as an integer. Every leg of both jobs has the one \
                 limit, about three times the slowest shard expected: an expression gives a leg \
                 a limit this contract cannot read, and a wider one lets a leg that never ends \
                 hold the required check and the merge-queue entry behind it for as long as it \
                 allows."
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
                "[test-job-toolchain] `{id}` has {} `dtolnay/rust-toolchain` steps, not one",
                toolchains.len()
            ));
            continue;
        }
        let with = field(toolchains[0], "with");
        let components: BTreeSet<&str> = with
            .and_then(|with| scalar(with, "components"))
            .map(|list| list.split(',').map(str::trim).collect())
            .unwrap_or_default();
        if !components.contains("clippy") {
            out.push(format!(
                "[test-job-toolchain] `{id}` installs components {components:?}, which do not \
                 include `clippy`, so `every_declared_effect_denial_refuses_for_the_reason\
                 _it_declares` cannot run there: `dtolnay/rust-toolchain` installs the minimal \
                 profile and `clippy-driver` is not in it."
            ));
        }
        if with.and_then(|with| scalar(with, "toolchain")) != Some("stable") {
            out.push(format!(
                "[test-job-toolchain] `{id}` selects toolchain {:?}, not `stable`",
                with.and_then(|with| scalar(with, "toolchain"))
            ));
        }
    }
    out
}

fn hosts_tests(target: &CiTarget) -> bool {
    target.runner != TEST_WINDOWS_PLATFORM
}

fn test_jobs_table_complaints() -> Vec<String> {
    let hosting: Vec<&str> = CI_TARGETS
        .iter()
        .filter(|target| hosts_tests(target))
        .map(|target| target.runner)
        .collect();
    let tabled: Vec<&str> = TEST_JOBS.iter().map(|row| row.runner).collect();
    if tabled == hosting {
        return Vec::new();
    }
    vec![format!(
        "[test-job-matrix] `TEST_JOBS` gives test jobs to the runners {tabled:?}, not, in order \
         and one job each, to {hosting:?} -- the `CI_TARGETS` runners that host tests. Each job \
         is held to its own row, so a platform the table leaves out leaves the workflow with \
         every check below still passing."
    )]
}

#[derive(Debug, PartialEq, Eq)]
struct TestLeg {
    leg: String,
    tests: String,
}

fn test_legs(row: &TestJob) -> Vec<TestLeg> {
    let selections = shard_selections(row.filters);
    let shards = selections.len();
    selections
        .into_iter()
        .enumerate()
        .map(|(index, tests)| TestLeg {
            leg: if shards == 1 {
                row.runner.to_owned()
            } else {
                format!("{}, {} of {shards}", row.runner, index + 1)
            },
            tests,
        })
        .collect()
}

fn test_leg(entry: &Yaml) -> Option<TestLeg> {
    if !field_complaints(entry, &["leg", "tests"], &[]).is_empty() {
        return None;
    }
    Some(TestLeg {
        leg: scalar(entry, "leg")?.to_owned(),
        tests: scalar(entry, "tests")?.to_owned(),
    })
}

fn test_leg_matrix_complaints(job: &Yaml, row: &TestJob) -> Vec<String> {
    let id = row.id;
    let mut out = Vec::new();
    let Some(strategy) = field(job, "strategy") else {
        out.push(format!(
            "[test-job-matrix] `{id}` declares no `strategy:`, so it is one leg with no shard \
             to run: the suite's selection reads a `matrix` that is not there"
        ));
        return out;
    };
    for complaint in field_complaints(strategy, &["fail-fast", "matrix"], &[]) {
        out.push(format!(
            "[test-job-matrix] `{id}`'s `strategy:` {complaint}. The legs are read from \
             `fail-fast` and `matrix` and from nothing else there."
        ));
    }
    if field(strategy, "fail-fast").and_then(Yaml::as_bool) != Some(false) {
        out.push(format!(
            "[test-job-matrix] `{id}`'s `fail-fast:` is not `false`, so one leg's failure \
             cancels the others before they report: a run shows one shard's failing tests and \
             hides the rest"
        ));
    }
    let matrix = field(strategy, "matrix");
    if let Some(matrix) = matrix {
        for complaint in field_complaints(matrix, &["include"], &[]) {
            out.push(format!(
                "[test-job-matrix] `{id}`'s `matrix:` {complaint}. The legs are the `include:` \
                 list and nothing else. Beside an axis GitHub adds an `include:` entry to the \
                 axis's combinations where it can instead of making a leg of it, a later entry \
                 overwriting the keys an earlier one added, and `exclude:` removes combinations \
                 of an axis; either way the legs are no longer the ones the list names."
            ));
        }
    }
    let expected: Vec<Option<TestLeg>> = test_legs(row).into_iter().map(Some).collect();
    let found: Option<Vec<Option<TestLeg>>> = matrix
        .and_then(|matrix| field(matrix, "include"))
        .and_then(Yaml::as_vec)
        .map(|entries| entries.iter().map(test_leg).collect());
    if found.as_ref() != Some(&expected) {
        out.push(format!(
            "[test-job-matrix] `{id}`'s `matrix.include:` reads as {found:#?}, not, in order, \
             exactly {expected:#?}. That is one mapping for each leg its row of `TEST_JOBS` \
             derives, with exactly the keys `leg` and `tests`: the runner alone where its suite \
             runs whole, otherwise the runner, a comma and the shard's place; and the libtest \
             arguments `shard_selections` derives from the row's filters, the empty string for \
             a suite that runs whole. An entry that reads as `None` is not such a mapping. A \
             dropped leg leaves its tests unrun on `{}` while the others are green -- the whole \
             suite, or one shard of it -- a narrowed one does the same for the tests it lost, a \
             widened one runs another shard's tests a second time, and a leg labelled for \
             another platform runs on this job's runner under that platform's name: Windows \
             has a suite job of its own, `{TEST_WINDOWS_HOSTED_JOB}`.",
            row.runner
        ));
    }
    out
}

fn installs_a_toolchain(step: &Yaml) -> bool {
    scalar(step, "uses").is_some_and(|uses| uses.starts_with(TOOLCHAIN_ACTION))
}

pub(super) fn ci_test_windows_hosted_job_complaints(doc: &Yaml) -> Vec<String> {
    let mut out = Vec::new();
    let Some(jobs) = field(doc, "jobs") else {
        return vec!["[jobs] the workflow declares no `jobs:` mapping".to_owned()];
    };
    let Some(job) = field(jobs, TEST_WINDOWS_HOSTED_JOB) else {
        return vec![format!(
            "[test-windows-hosted-missing] no `{TEST_WINDOWS_HOSTED_JOB}` job, so no job then \
             runs the Windows suite"
        )];
    };
    let Some(platform) = CI_TARGETS
        .iter()
        .find(|target| target.runner == TEST_WINDOWS_PLATFORM)
    else {
        return vec![format!(
            "[test-windows-hosted-platform] `{TEST_WINDOWS_PLATFORM}` is not a runner this \
             contract models, so the shell its steps resolve to is undecidable here"
        )];
    };

    for complaint in field_complaints(
        job,
        &TEST_WINDOWS_HOSTED_JOB_FIELDS,
        &OPTIONAL_DEFAULTS_FIELD,
    ) {
        out.push(format!(
            "[unexpected-job-field] `{TEST_WINDOWS_HOSTED_JOB}` {complaint}"
        ));
    }
    if scalar(job, "name") != Some(TEST_WINDOWS_HOSTED_NAME) {
        out.push(format!(
            "[test-windows-hosted-name] `{TEST_WINDOWS_HOSTED_JOB}` is named {:?}, not \
             `{TEST_WINDOWS_HOSTED_NAME}`",
            scalar(job, "name")
        ));
    }
    if scalar(job, "runs-on") != Some(TEST_WINDOWS_PLATFORM) {
        out.push(format!(
            "[test-windows-hosted-runner] `{TEST_WINDOWS_HOSTED_JOB}` runs on {:?}, not \
             `{TEST_WINDOWS_PLATFORM}`. The suite runs on GitHub's runner on every event: a \
             label set names a self-hosted machine, and an expression a runner this contract \
             cannot read.",
            field(job, "runs-on")
        ));
    }
    let minutes = field(job, "timeout-minutes").and_then(Yaml::as_i64);
    if minutes != Some(TEST_WINDOWS_HOSTED_TIMEOUT_MINUTES) {
        out.push(format!(
            "[test-windows-hosted-timeout] `{TEST_WINDOWS_HOSTED_JOB}` is bounded at {minutes:?} \
             minutes, not exactly {TEST_WINDOWS_HOSTED_TIMEOUT_MINUTES}"
        ));
    }
    out.extend(windows_shard_matrix_complaints(job));

    for (index, step) in steps_of(job).iter().enumerate() {
        let named = format!("`{TEST_WINDOWS_HOSTED_JOB}` step {index}");
        let runs_the_suite = scalar(step, "run") == Some(WINDOWS_SHARD_WITNESS);
        let runs_the_capture = scalar(step, "run") == Some(HOST_ACCEPTANCE_WINDOWS_CAPTURE);
        let runs_the_observation = scalar(step, "run") == Some(CAPABILITY_WINDOWS_OBSERVATION);
        let allowed: &[&str] = if runs_the_suite || runs_the_capture {
            &TEST_STEP_FIELDS
        } else if runs_the_observation {
            &CAPABILITY_STEP_FIELDS
        } else {
            &STEP_FIELDS
        };
        let strange = unexpected(&field_names(step), allowed);
        if !strange.is_empty() {
            out.push(format!(
                "[unexpected-step-field] {named} declares {strange:?}. No step of this job may \
                 carry an `if:`: on the step that runs the suite it is a job that reports \
                 success having run nothing for the events it excludes, and on any other step \
                 it is a script or an action one event executes and the pull-request build \
                 never shows."
            ));
        }
        if runs_the_suite {
            out.extend(step_env_complaints(
                step,
                &named,
                "test-windows-hosted-step-env",
                &TEST_WINDOWS_STEP_ENV,
            ));
        }
        if runs_the_capture {
            out.extend(step_env_complaints(
                step,
                &named,
                "test-windows-hosted-capture-step-env",
                &TEST_WINDOWS_STEP_ENV,
            ));
        }
        if runs_the_observation {
            out.extend(step_env_complaints(
                step,
                &named,
                "test-windows-hosted-capability-step-env",
                &TEST_WINDOWS_STEP_ENV,
            ));
            out.extend(observation_step_complaints(
                step,
                &named,
                "test-windows-hosted-capability-step",
            ));
        }
        if scalar(step, "run").is_some() {
            out.extend(shell_complaints(
                doc,
                job,
                step,
                platform,
                &named,
                platform.default_shell,
            ));
        }
    }

    let running = steps_of(job)
        .iter()
        .filter(|step| scalar(step, "run") == Some(WINDOWS_SHARD_WITNESS))
        .count();
    if running != 1 {
        out.push(format!(
            "[test-windows-hosted-command] `{TEST_WINDOWS_HOSTED_JOB}` has {running} steps whose \
             `run:` is exactly the pinned shard suite-and-witness script, not one. The lines \
             before the build refuse any compiler but the image's `{GOLDEN_IMAGE_TOOLCHAIN}`, \
             which on this runner is the one the install selected only if its `rustup default` \
             took. The build is `{TEST_COMMAND} --no-run`, exited on when it fails, and \
             `RUSTC_BOOTSTRAP` is set only after it and no `cargo` follows: without `--no-run` \
             Cargo runs the whole suite itself, with no times, and with the variable set for \
             it the build is not the one the other legs make. Every test harness the build \
             reported is then run in turn, an empty list refused, each handed the leg's \
             `matrix.tests` and then `-Z unstable-options --report-time`, without which every \
             leg runs the whole suite or reports no test's time. The loop keeps each harness's \
             code as soon as `WaitForExit()` returns, before it copies the harness's two \
             outputs, each copy in a `try` of its own whose `catch` prints the failure and \
             throws only where the harness passed, so a copy that fails cannot end the step \
             before the code is kept and the errors are printed; it stops at the first harness \
             that fails and keeps its code for the step's exit. The lines after it are \
             the count of passing tests against the leg's `matrix.floor`, without which a shard \
             that compiled and executed nothing is green here as a whole suite that executed \
             nothing would be."
        ));
    }
    out.extend(capture_complaints(
        job,
        TEST_WINDOWS_HOSTED_JOB,
        "test-windows-hosted-capture-command",
        WINDOWS_SHARD_WITNESS,
        HOST_ACCEPTANCE_WINDOWS_CAPTURE,
    ));
    out.extend(observation_complaints(
        job,
        TEST_WINDOWS_HOSTED_JOB,
        "test-windows-hosted-capability-command",
        HOST_ACCEPTANCE_WINDOWS_CAPTURE,
        CAPABILITY_WINDOWS_OBSERVATION,
    ));
    out.extend(checkout_complaints(
        job,
        TEST_WINDOWS_HOSTED_JOB,
        "test-windows-hosted-checkout",
    ));
    out.extend(step_pin_complaints(
        job,
        TEST_WINDOWS_HOSTED_JOB,
        "test-windows-hosted-run-script",
        &TEST_WINDOWS_HOSTED_SCRIPTS,
    ));
    out.extend(toolchain_complaints(
        job,
        TEST_WINDOWS_HOSTED_JOB,
        "test-windows-hosted-toolchain",
        None,
    ));
    for (index, step) in steps_of(job).iter().enumerate() {
        if !installs_a_toolchain(step) {
            continue;
        }
        let selected = field(step, "with").and_then(|with| scalar(with, "toolchain"));
        if selected != Some(GOLDEN_IMAGE_TOOLCHAIN) {
            out.push(format!(
                "[test-windows-hosted-toolchain] `{TEST_WINDOWS_HOSTED_JOB}` step {index} \
                 installs toolchain {selected:?}, not `{GOLDEN_IMAGE_TOOLCHAIN}`, the compiler \
                 the guest's image carried when the guest was retired. The pin is kept so that \
                 the suite's compiler moves by an edit -- the install, the two lines of the \
                 suite step that refuse any other compiler and this contract's constant, \
                 together -- and not with GitHub's stable: under `stable` a test whose outcome \
                 depends on the compiler -- a version-gated test, a std behaviour change, a new \
                 lint in test code under `-D warnings` -- moves with GitHub's image, and \
                 `lint (windows)` already compiles, code-generates and links the tree on \
                 current stable on every event. With the suite step intact its compiler \
                 questions throw on every run, so this is the drift between the two places \
                 the workflow writes the version, refused before it is pushed."
            ));
        }
    }
    for (index, step) in steps_of(job).iter().enumerate() {
        if scalar(step, "uses").is_some_and(|uses| uses.starts_with(CACHE_ACTION)) {
            out.push(format!(
                "[test-windows-hosted-cache] `{TEST_WINDOWS_HOSTED_JOB}` step {index} restores and \
                 saves a cargo cache. The job carries none. Until 2026-11-20 the reason was that \
                 `master` saved a twin cache only on a push the picker sent here, so nearly \
                 every run would upload a cache nothing later read. Every push to `master` now \
                 runs these legs, so `master` would save one that pull requests and queue \
                 entries could restore. Whether that pays inside a leg's \
                 {TEST_WINDOWS_HOSTED_TIMEOUT_MINUTES} minutes is not measured, and adding a \
                 cache step is a change of its own, made together with this refusal."
            ));
        }
    }

    let components: BTreeSet<&str> = steps_of(job)
        .iter()
        .filter(|step| installs_a_toolchain(step))
        .filter_map(|step| field(step, "with").and_then(|with| scalar(with, "components")))
        .flat_map(|list| list.split(',').map(str::trim))
        .collect();
    if !components.contains("clippy") {
        out.push(format!(
            "[test-windows-hosted-toolchain] `{TEST_WINDOWS_HOSTED_JOB}` installs components \
             {components:?}, which do not include `clippy`. The suite's effect-denial fixtures \
             run `clippy-driver`, and `dtolnay/rust-toolchain` installs the minimal profile."
        ));
    }
    out
}

#[derive(Debug, PartialEq, Eq)]
struct ShardLeg {
    shard: String,
    tests: String,
    floor: i64,
}

pub(super) fn windows_shard_labels() -> Vec<String> {
    let shards = WINDOWS_SHARD_FILTERS.len() + 1;
    (1..=shards)
        .map(|shard| format!("{shard} of {shards}"))
        .collect()
}

fn windows_shard_legs() -> Vec<ShardLeg> {
    windows_shard_labels()
        .into_iter()
        .zip(shard_selections(&WINDOWS_SHARD_FILTERS))
        .zip(WINDOWS_SHARD_FLOORS)
        .map(|((shard, tests), floor)| ShardLeg {
            shard,
            tests,
            floor: i64::from(floor),
        })
        .collect()
}

fn shard_leg(entry: &Yaml) -> Option<ShardLeg> {
    if !field_complaints(entry, &["floor", "shard", "tests"], &[]).is_empty() {
        return None;
    }
    Some(ShardLeg {
        shard: scalar(entry, "shard")?.to_owned(),
        tests: scalar(entry, "tests")?.to_owned(),
        floor: field(entry, "floor")?.as_i64()?,
    })
}

fn windows_shard_matrix_complaints(job: &Yaml) -> Vec<String> {
    let mut out = Vec::new();
    let Some(strategy) = field(job, "strategy") else {
        return vec![format!(
            "[test-windows-hosted-matrix] `{TEST_WINDOWS_HOSTED_JOB}` declares no `strategy:`, \
             so it is one leg with no shard to run: its script's `matrix` expressions are \
             empty, and no table divides the suite"
        )];
    };
    for complaint in field_complaints(strategy, &["fail-fast", "matrix"], &[]) {
        out.push(format!(
            "[test-windows-hosted-matrix] `{TEST_WINDOWS_HOSTED_JOB}`'s `strategy:` {complaint}. \
             The shards are read from `fail-fast` and `matrix` and from nothing else there."
        ));
    }
    if field(strategy, "fail-fast").and_then(Yaml::as_bool) != Some(false) {
        out.push(format!(
            "[test-windows-hosted-matrix] `{TEST_WINDOWS_HOSTED_JOB}`'s `fail-fast:` is not \
             `false`, so one leg's failure cancels the others before they report: a run shows \
             one shard's failing tests and hides every other shard's."
        ));
    }
    let matrix = field(strategy, "matrix");
    if let Some(matrix) = matrix {
        for complaint in field_complaints(matrix, &["include"], &[]) {
            out.push(format!(
                "[test-windows-hosted-matrix] `{TEST_WINDOWS_HOSTED_JOB}`'s `matrix:` \
                 {complaint}. The legs are the `include:` list and nothing else. Beside an axis \
                 GitHub adds each `include:` entry to the axis's combinations instead of making \
                 a leg of it, a later entry overwriting the keys an earlier one added, so the \
                 legs are no longer the shards the list names."
            ));
        }
    }
    let expected: Vec<Option<ShardLeg>> = windows_shard_legs().into_iter().map(Some).collect();
    let found: Option<Vec<Option<ShardLeg>>> = matrix
        .and_then(|matrix| field(matrix, "include"))
        .and_then(Yaml::as_vec)
        .map(|entries| entries.iter().map(shard_leg).collect());
    if found.as_ref() != Some(&expected) {
        out.push(format!(
            "[test-windows-hosted-matrix] `{TEST_WINDOWS_HOSTED_JOB}`'s `matrix.include:` reads \
             as {found:#?}, not, in order, exactly {expected:#?}. That is one mapping for each \
             shard, with exactly the keys `shard`, `tests` and `floor`: the shard's place as a \
             string, the libtest arguments `shard_selections` derives for it from \
             `WINDOWS_SHARD_FILTERS`, and its floor in `WINDOWS_SHARD_FLOORS` as an integer; an \
             entry that reads as `None` is not such a mapping. A dropped or narrowed shard runs \
             its tests nowhere while the other legs are green, a widened one runs another \
             shard's tests a second time, and a floor below the table's is one a shard that \
             executed almost none of its tests clears."
        ));
    }
    out
}

/// Exactly one step of `job` runs the pinned host acceptance `capture`, and it is
/// the step immediately after the one step that runs the pinned `suite`.
fn capture_complaints(
    job: &Yaml,
    named: &str,
    code: &str,
    suite: &str,
    capture: &str,
) -> Vec<String> {
    let at = |script: &str| -> Vec<usize> {
        steps_of(job)
            .iter()
            .enumerate()
            .filter(|(_, step)| scalar(step, "run") == Some(script))
            .map(|(index, _)| index)
            .collect()
    };
    let suites = at(suite);
    let captures = at(capture);
    match (suites.as_slice(), captures.as_slice()) {
        ([suite_at], [capture_at]) if *capture_at == suite_at + 1 => Vec::new(),
        _ => vec![format!(
            "[{code}] `{named}` runs the pinned host acceptance capture at steps {captures:?} \
             and its pinned suite at steps {suites:?}, not one capture immediately after one \
             suite. The capture is the native HOST acceptance record: deleted, filtered to \
             another or no test, echoed, or with its exit, its zero-test refusal or its frame \
             check edited out, it is no step this contract pins, and a job that reports success \
             leaves no record or a record nothing refused. Immediately after the suite, because \
             a step between them -- a cache restore, a checkout -- changes the harness or the \
             tree the capture runs after the suite ran."
        )],
    }
}

/// Exactly one step of `job` runs the pinned container capability `observation`, and it is
/// the step immediately after the one step that runs the pinned host acceptance `capture`.
fn observation_complaints(
    job: &Yaml,
    named: &str,
    code: &str,
    capture: &str,
    observation: &str,
) -> Vec<String> {
    let at = |script: &str| -> Vec<usize> {
        steps_of(job)
            .iter()
            .enumerate()
            .filter(|(_, step)| scalar(step, "run") == Some(script))
            .map(|(index, _)| index)
            .collect()
    };
    let captures = at(capture);
    let observations = at(observation);
    match (captures.as_slice(), observations.as_slice()) {
        ([capture_at], [observation_at]) if *observation_at == capture_at + 1 => Vec::new(),
        _ => vec![format!(
            "[{code}] `{named}` runs the pinned container capability observation at steps \
             {observations:?} and its pinned host acceptance capture at steps {captures:?}, not \
             one observation immediately after one capture. The observation is the platform's \
             container capability record: deleted, filtered to another or no test, echoed, or \
             with its exit, its zero-test refusal or its frame check edited out, it is no step \
             this contract pins, and a job that reports success leaves no record or a record \
             nothing refused. Immediately after the capture, because a step between them -- a \
             cache restore, a checkout -- changes the harness or the tree the observation runs \
             after the suite and the capture ran, and above the capture it would run before \
             the record that step makes."
        )],
    }
}

/// The observation step's own fields: exactly `env`, `name`, `run` and `timeout-minutes`,
/// the bound exactly [`CAPABILITY_STEP_TIMEOUT_MINUTES`].
fn observation_step_complaints(step: &Yaml, named: &str, code: &str) -> Vec<String> {
    let mut out: Vec<String> = field_complaints(step, &CAPABILITY_STEP_FIELDS, &[])
        .into_iter()
        .map(|complaint| format!("[{code}] {named} {complaint}"))
        .collect();
    let minutes = field(step, "timeout-minutes").and_then(Yaml::as_i64);
    if minutes != Some(CAPABILITY_STEP_TIMEOUT_MINUTES) {
        out.push(format!(
            "[{code}] {named} bounds the container capability observation at {minutes:?} \
             minutes, not exactly {CAPABILITY_STEP_TIMEOUT_MINUTES}. The bound covers the \
             questions' requested 135 s with Cargo's start; without it a stalled question is \
             held by the job's own timeout, and a wider one spends the leg's headroom."
        ));
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
            if let Some(targets) = scalar(inputs, "targets") {
                if !TOOLCHAIN_TARGETS.contains(&targets) {
                    out.push(format!(
                        "[unpinned-action-input] `{named}` step {index} asks for targets \
                         {targets:?}, which is not one of {TOOLCHAIN_TARGETS:?}. This action \
                         interpolates the value into a shell command, as it does components, \
                         so an unpinned one runs whatever it contains."
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
         with -- and the runner's destruction after the job recalls nothing."
    )]
}

fn job_permissions_complaints(doc: &Yaml) -> Vec<String> {
    let mut out = Vec::new();
    let Some(jobs) = field(doc, "jobs") else {
        return out;
    };
    for name in field_names(jobs) {
        if field(jobs, &name).is_some_and(|job| field(job, "permissions").is_some()) {
            out.push(format!(
                "[job-permissions] `{name}` declares its own `permissions:`. No job of this \
                 workflow declares one: a block on a job replaces the workflow's for that job, \
                 and hands whatever it names to a job that runs the candidate's build scripts \
                 and tests."
            ));
        }
    }
    out
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
             Clippy stop before codegen, and the Windows suite's shards build with the pinned \
             `{GOLDEN_IMAGE_TOOLCHAIN}`."
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

fn msrv_entry(entry: &Yaml) -> Option<(String, String)> {
    if !field_complaints(entry, &MSRV_ENTRY_KEYS, &[]).is_empty() {
        return None;
    }
    Some((
        scalar(entry, "os")?.to_owned(),
        scalar(entry, "target")?.to_owned(),
    ))
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
            let check_runners: BTreeSet<&str> = CI_TARGETS
                .iter()
                .map(|target| target.check_runner)
                .collect();
            for runner in check_runners {
                let Some(host) = runner_target(runner) else {
                    out.push(format!(
                        "[step-shell] `{MSRV_JOB}` step {index} runs on `{runner}`, which is not \
                         a runner this contract models, so the shell it resolves to is \
                         undecidable here"
                    ));
                    continue;
                };
                out.extend(shell_complaints(
                    doc,
                    job,
                    step,
                    host,
                    &format!("`{MSRV_JOB}` step {index} on `{runner}`"),
                    host.default_shell,
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
             the exact pins this manifest carries for the floor; dropping `--target` checks \
             the runner's own tuple, so the entry for a target checked from another runner's \
             host compiles that host's regions and none of its own; and an `echo` satisfies \
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

    let expected_entries: BTreeSet<(String, String)> = CI_TARGETS
        .iter()
        .map(|target| (target.check_runner.to_owned(), target.triple.to_owned()))
        .collect();
    match field(job, "strategy") {
        None => out.push(format!(
            "[msrv-job-matrix] the `{MSRV_JOB}` job declares no `strategy:`, so it checks the \
             floor for one target"
        )),
        Some(strategy) => {
            for complaint in field_complaints(strategy, &["fail-fast", "matrix"], &[]) {
                out.push(format!(
                    "[msrv-job-matrix] the `{MSRV_JOB}` job's `strategy:` {complaint}. The \
                     entries are read from `fail-fast` and `matrix` and from nothing else there."
                ));
            }
            if field(strategy, "fail-fast").and_then(Yaml::as_bool) != Some(false) {
                out.push(format!(
                    "[msrv-job-matrix] the `{MSRV_JOB}` job's `fail-fast:` is not `false`, so \
                     one target's floor failure cancels the other two before they report"
                ));
            }
            if let Some(matrix) = field(strategy, "matrix") {
                for complaint in field_complaints(matrix, &MSRV_MATRIX_FIELDS, &[]) {
                    out.push(format!(
                        "[msrv-job-matrix] the `{MSRV_JOB}` job's `matrix:` {complaint}. An axis \
                         or `exclude:` next to `include:` changes which combinations run: beside \
                         an axis GitHub folds an `include:` entry into the axis's combinations \
                         where it can, and `exclude:` removes combinations, so the entries that \
                         run are no longer the ones the list names."
                    ));
                }
            }
        }
    }
    let listed: Option<Vec<Option<(String, String)>>> = field(job, "strategy")
        .and_then(|strategy| field(strategy, "matrix"))
        .and_then(|matrix| field(matrix, "include"))
        .and_then(Yaml::as_vec)
        .map(|entries| entries.iter().map(msrv_entry).collect());
    let found: BTreeSet<(String, String)> = listed.iter().flatten().flatten().cloned().collect();
    let exact = listed
        .as_ref()
        .is_some_and(|entries| entries.len() == found.len() && entries.iter().all(Option::is_some));
    if !exact || found != expected_entries {
        out.push(format!(
            "[msrv-job-matrix] the `{MSRV_JOB}` job's `matrix.include:` reads as {listed:?}, \
             the `(os, target)` pairs {found:?}, not exactly {expected_entries:?}, each once: \
             one entry for each target in `CI_TARGETS`, with exactly the keys {MSRV_ENTRY_KEYS:?}, \
             the runner that checks the target and its triple. An entry that reads as `None` \
             is not such a mapping. A dropped or retargeted entry leaves a target's floor \
             unchecked while the other entries are green, and an entry twice checks one target \
             twice in its place."
        ));
    }
    if scalar(job, "runs-on") != Some(MSRV_RUNS_ON) {
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
    let targets = field(toolchains[0], "with").and_then(|with| scalar(with, "targets"));
    if targets != Some(MSRV_TOOLCHAIN_TARGETS) {
        out.push(format!(
            "[msrv-job-toolchain] the `{MSRV_JOB}` job's toolchain step installs the targets \
             {targets:?}, not exactly `{MSRV_TOOLCHAIN_TARGETS}`: the entry's own triple, \
             which its check names with `--target`. An entry whose target is not the runner's \
             own has no standard library for it without this input."
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
    out.extend(ci_test_windows_hosted_job_complaints(doc));
    out.extend(job_permissions_complaints(doc));
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
        anchor: "      - run: cargo clippy --target aarch64-apple-darwin --all-targets --all-features -- -D warnings\n",
        replacement: "      - run: echo cargo clippy --target aarch64-apple-darwin --all-targets --all-features -- -D warnings\n",
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
        anchor: "    runs-on: ubuntu-latest\n",
        replacement: "    runs-on: ubuntu-latest\n    if: ${{ false }}\n",
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
        escape: "a step-level environment retargets the Windows gate's compile: \
                 `CARGO_BUILD_TARGET` names the Linux tuple, and the `run:` scalar still \
                 matches character for character. Where that tuple's standard library is \
                 installed the job is green and Clippy examined a target whose `#[cfg]` bodies \
                 are not Windows'; where it is not, the job fails for want of it. The field is \
                 refused before either. The row stands on `lint (windows)` because \
                 `lint (macos)` names its tuple with `--target`, which Cargo takes over the \
                 variable.",
        job: Some("lint-windows"),
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
        anchor: "      - run: cargo clippy --target aarch64-apple-darwin --all-targets --all-features -- -D warnings\n",
        replacement: "      - run: cargo clippy --target aarch64-apple-darwin --all-targets --all-features -- -D warnings\n\
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
        escape: "an `exclude:` naming the first macOS leg, beside the `include:` list in \
                 `test-macos` that still names all five. Over an axis, `exclude:` removes \
                 combinations the axis still lists, and every check that reads the list passes \
                 while the fixtures never run on that platform -- the matrix half of \
                 `PR5-MACOS-CLIPPY-NEVER-RUN`, and what this row measured while the legs were \
                 an `os:` axis. GitHub documents that `include:` entries are processed after \
                 `exclude:`, so by that rule this one removes no leg of an `include:` list; \
                 that is the rule as documented, and no workflow was run to watch it. The \
                 oracle does not weigh it: the legs are read from `include:` and from nothing \
                 else in `matrix:`, so the key is refused whatever it would remove.",
        job: Some("test-macos"),
        anchor: "        include:\n",
        replacement: "        exclude:\n\
                      \x20         - leg: \"macos-latest, 1 of 5\"\n\
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
        escape: "the macOS gate's job moved off its check runner, to Windows. The command \
                 still names its tuple, but every shell and environment reading of the job \
                 was made for `ubuntu-latest` -- on Windows its step resolves to `pwsh`, where \
                 the process environment is case-insensitive -- so the contract pins the \
                 runner with the command, and no job on `ubuntu-latest` runs the macOS cross \
                 gate. `PR5-MACOS-CLIPPY-NEVER-RUN` is the ledger row for the state the gate \
                 restores.",
        job: Some("lint-macos"),
        anchor: "    runs-on: ubuntu-latest\n",
        replacement: "    runs-on: windows-latest\n",
        refused_as: "no-gate-job",
    },
    WorkflowEscape {
        name: "MUT-GATE-CROSS-TARGET-DROPPED",
        escape: "the macOS gate's run line loses `--target aarch64-apple-darwin`. Two jobs then \
                 run Clippy over the Linux tuple and none compiles the macOS regions -- \
                 `PR5-MACOS-CLIPPY-NEVER-RUN` one flag away, with the job, its runner and its \
                 green result all unchanged.",
        job: Some("lint-macos"),
        anchor: "      - run: cargo clippy --target aarch64-apple-darwin --all-targets --all-features -- -D warnings\n",
        replacement: "      - run: cargo clippy --all-targets --all-features -- -D warnings\n",
        refused_as: "no-gate-job",
    },
    WorkflowEscape {
        name: "MUT-GATE-TOOLCHAIN-TARGET-DROPPED",
        escape: "the macOS gate's toolchain step no longer installs `aarch64-apple-darwin`. \
                 The job then fails for want of that tuple's standard library, which is loud; \
                 the pin keeps the tuple the toolchain installs in one place with the command \
                 that names it.",
        job: Some("lint-macos"),
        anchor: "          targets: aarch64-apple-darwin\n",
        replacement: "",
        refused_as: "gate-toolchain-target",
    },
    WorkflowEscape {
        name: "MUT-AGGREGATE-NEEDS-DROPPED",
        escape: "a gate the aggregate does not depend on: branch protection settles green \
                 while the Windows denial gate is red. `PR5D-MSVC-CLIPPY-NEVER-RUN`.",
        job: Some("merge-gate"),
        anchor: "    needs: [lint, lint-windows, lint-macos, msrv, test-linux, test-macos, test-windows-hosted]\n",
        replacement: "    needs: [lint, lint-macos, msrv, test-linux, test-macos, test-windows-hosted]\n",
        refused_as: "aggregate-needs",
    },
    WorkflowEscape {
        name: "MUT-AGGREGATE-DROPS-TEST-MACOS",
        escape: "the aggregate no longer needs `test-macos`: branch protection settles green \
                 while a macOS shard is red, or before the macOS legs have run at all. The split \
                 made the macOS suite a job of its own, so the aggregate must name it as it \
                 names every other gate.",
        job: Some("merge-gate"),
        anchor: "    needs: [lint, lint-windows, lint-macos, msrv, test-linux, test-macos, test-windows-hosted]\n",
        replacement: "    needs: [lint, lint-windows, lint-macos, msrv, test-linux, test-windows-hosted]\n",
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
                 `requires.split_whitespace().any(|word| word == looped)` passes on the \
                 trailing mention.",
        job: Some("merge-gate"),
        anchor: "          for gate in LINT LINT_WINDOWS LINT_MACOS MSRV TEST_LINUX TEST_MACOS TEST_WINDOWS_HOSTED; do\n",
        replacement: "          for gate in LINT LINT_WINDOWS MSRV TEST_LINUX TEST_MACOS TEST_WINDOWS_HOSTED; do : LINT_MACOS\n",
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
        job: Some("test-linux"),
        anchor: "      - run: cargo test --all-targets --all-features -- ${{ matrix.tests }}\n",
        replacement: "",
        refused_as: "test-job-command",
    },
    WorkflowEscape {
        name: "MUT-CI-CARGO-TEST-STEP-SKIPPED",
        escape: "the same, by disabling the job rather than deleting the step. The other \
                 mutation `test-docs-consistency.sh` kept as history.",
        job: Some("test-linux"),
        anchor: "    runs-on: ubuntu-latest\n",
        replacement: "    runs-on: ubuntu-latest\n    if: false\n",
        refused_as: "unexpected-job-field",
    },
    WorkflowEscape {
        name: "MUT-CI-STOPS-INSTALLING-CLIPPY",
        escape: "`clippy-driver` is a test dependency of the job that runs these fixtures. \
                 The first version of that test looked for the word `clippy` in the job's \
                 text and the nine-line comment above the line spelled it, so deleting the \
                 line left the test green -- `PR4-CENSUS-COMMENT-ORACLE`, in the test whose \
                 purpose is to answer which command runs this.",
        job: Some("test-linux"),
        anchor: "          components: clippy\n",
        replacement: "",
        refused_as: "test-job-toolchain",
    },
    WorkflowEscape {
        name: "MUT-CI-TEST-MATRIX-NARROWED",
        escape: "the hosted fixtures run on one platform and the other goes unexercised, \
                 while every check that names the command still passes. The legs removed from \
                 `test-linux`'s `include:` list are its two, their four lines, which are that \
                 platform's whole suite: the five macOS legs of `test-macos` stay, each green.",
        job: Some("test-linux"),
        anchor: "          - leg: \"ubuntu-latest, 1 of 2\"\n\
                 \x20           tests: \"engine::\"\n\
                 \x20         - leg: \"ubuntu-latest, 2 of 2\"\n\
                 \x20           tests: \"--skip engine::\"\n",
        replacement: "",
        refused_as: "test-job-matrix",
    },
    WorkflowEscape {
        name: "MUT-TEST-MATRIX-KEEPS-WINDOWS",
        escape: "`test-linux`'s matrix quietly re-admits Windows, as a third `include:` entry \
                 labelled `windows-latest` that runs the suite whole. The job's runner is \
                 `ubuntu-latest`, so the whole suite runs once more on Linux, on the critical \
                 path, under the check name `test (windows-latest)`, which reads as a Windows \
                 result beside the fourteen legs of the job that runs the Windows suite, with \
                 every other check passing.",
        job: Some("test-linux"),
        anchor: "        include:\n",
        replacement: "        include:\n\
                      \x20         - leg: \"windows-latest\"\n\
                      \x20           tests: \"\"\n",
        refused_as: "test-job-matrix",
    },
    WorkflowEscape {
        name: "MUT-TEST-RUN-RETARGETED",
        escape: "the same retarget on `test-linux`: the Linux legs test `master` while every \
                 pin still matches",
        job: Some("test-linux"),
        anchor: "          git config --global user.name \"upstroke CI\"\n",
        replacement: "          git config --global user.name \"upstroke CI\"\n\
                      \x20         git fetch origin master\n\
                      \x20         git checkout --detach FETCH_HEAD\n",
        refused_as: "test-job-run-script",
    },
    WorkflowEscape {
        name: "MUT-TEST-CASEFOLD-DECLARATION-DROPPED",
        escape: "the hosted test step no longer declares that the macOS runner's temporary \
                 directory folds case, so the alias test observes its native branch there \
                 instead of requiring it: a folding temporary directory is no longer a \
                 prerequisite of the leg, and on a temporary directory that does not fold \
                 case the undeclared test passes with the branch unrun where the declared \
                 one fails at the declaration (34-r6-casefold-declaration.log, ext4) -- while \
                 every pin still matches",
        job: Some("test-macos"),
        anchor: "      - run: cargo test --all-targets --all-features -- ${{ matrix.tests }}\n\
                 \x20       env:\n\
                 \x20         UPSTROKE_TEST_TEMP_FOLDS_CASE: \"1\"\n",
        replacement: "      - run: cargo test --all-targets --all-features -- ${{ matrix.tests }}\n",
        refused_as: "test-step-env",
    },
    WorkflowEscape {
        name: "MUT-TEST-CASEFOLD-DECLARED-ON-THE-WRONG-LEG",
        escape: "the declaration made on the ubuntu runner, whose temporary directory does not \
                 fold case: `test-linux`'s suite step declares `1`, and each Linux leg fails \
                 its declaration loudly -- a red that reads as a flake rather than as the guard \
                 declared on the wrong platform. Under one job this row moved the declaration \
                 from the macOS legs to the ubuntu ones in one expression; with a job per \
                 platform each job's map is its own, and this is the wrong one of the two",
        job: Some("test-linux"),
        anchor: "      - run: cargo test --all-targets --all-features -- ${{ matrix.tests }}\n\
                 \x20       env:\n\
                 \x20         UPSTROKE_TEST_TEMP_FOLDS_CASE: \"\"\n",
        replacement: "      - run: cargo test --all-targets --all-features -- ${{ matrix.tests }}\n\
                      \x20       env:\n\
                      \x20         UPSTROKE_TEST_TEMP_FOLDS_CASE: \"1\"\n",
        refused_as: "test-step-env",
    },
    WorkflowEscape {
        name: "MUT-TEST-STEP-RETARGETED-THROUGH-THE-ADMITTED-ENV",
        escape: "the hosted test step's `env:`, one of the step-level maps this contract \
                 admits, gains a second key: \
                 `CARGO_BUILD_TARGET` retargets the compile the suite performs while the \
                 `run:` scalar and the declaration both still match -- \
                 `MUT-GATE-STEP-RETARGETED`'s shape on the step that now carries an \
                 environment",
        job: Some("test-macos"),
        anchor: "      - run: cargo test --all-targets --all-features -- ${{ matrix.tests }}\n\
                 \x20       env:\n\
                 \x20         UPSTROKE_TEST_TEMP_FOLDS_CASE: \"1\"\n",
        replacement: "      - run: cargo test --all-targets --all-features -- ${{ matrix.tests }}\n\
                      \x20       env:\n\
                      \x20         UPSTROKE_TEST_TEMP_FOLDS_CASE: \"1\"\n\
                      \x20         CARGO_BUILD_TARGET: x86_64-unknown-linux-gnu\n",
        refused_as: "test-step-env",
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
        anchor: "      - run: cargo check --locked --target ${{ matrix.target }} --all-targets --all-features\n",
        replacement: "      - run: git fetch origin master && git checkout --detach FETCH_HEAD\n\
                      \x20     - run: cargo check --locked --target ${{ matrix.target }} --all-targets --all-features\n",
        refused_as: "msrv-run-script",
    },
    WorkflowEscape {
        name: "MUT-STEP-USES-UNPINNED",
        escape: "the Windows suite job's checkout floats to `actions/checkout@v4`: the same \
                 action at whatever commit the tag points to, which is code nobody here \
                 reviewed with a checkout of its own",
        job: Some("test-windows-hosted"),
        anchor: "      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0\n",
        replacement: "      - uses: actions/checkout@v4\n",
        refused_as: "unpinned-action",
    },
    WorkflowEscape {
        name: "MUT-GATE-TOOLCHAIN-DOWNGRADED",
        escape: "the Windows gate installs `1.97.1`, the compiler the suite's shards pin, \
                 instead of `stable`: the build witness then links with the compiler the shards \
                 already link with, so no leg compiles the Windows tree on current stable and a \
                 build script emitting a bad link directive only on newer rustc goes green",
        job: Some("lint-windows"),
        anchor: "          toolchain: stable\n",
        replacement: "          toolchain: 1.97.1\n",
        refused_as: "gate-toolchain",
    },
    WorkflowEscape {
        name: "MUT-DEFAULTS-WORKING-DIRECTORY",
        escape: "a job-level default working directory points every `run:` step at another \
                 crate: the pinned test command tests an empty `ci-pass/` while the runner, \
                 shells, checkout and command all still match character for character",
        job: Some("test-windows-hosted"),
        anchor: "    timeout-minutes: 40\n",
        replacement: "    timeout-minutes: 40\n\
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
                 credential configured for later Git commands, and GitHub discarding the runner \
                 after the job recalls nothing",
        job: None,
        anchor: "permissions:\n  contents: read\n",
        replacement: "permissions: write-all\n",
        refused_as: "workflow-permissions",
    },
    WorkflowEscape {
        name: "MUT-WORKFLOW-ENV-TARGET-RUNNER",
        escape: "a Cargo target runner at workflow scope: a harness that `cargo test` runs on a \
                 Windows leg compiles and does not execute, because Cargo hands it to \
                 `cmd /c echo`, which prints the path it was given and exits zero. That was \
                 every Windows test harness until the suite step began starting the harnesses \
                 itself, which a runner does not reach; it is still the two capture steps \
                 after it, which run `cargo test`. No Unix leg reads a Windows-target \
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
                 witness links a tree the candidate never touched while the suite's shards, on \
                 the pinned `1.97.1`, link and pass the candidate",
        job: Some("lint-windows"),
        anchor: "      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0\n",
        replacement: "      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0\n\
                      \x20       with:\n\
                      \x20         ref: master\n",
        refused_as: "gate-checkout",
    },
    WorkflowEscape {
        name: "MUT-TEST-CHECKOUT-REF",
        escape: "`test-linux` checks out `master`: the Linux legs test a tree the candidate \
                 never touched while Clippy and MSRV read the candidate",
        job: Some("test-linux"),
        anchor: "      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0\n",
        replacement: "      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0\n\
                      \x20       with:\n\
                      \x20         ref: master\n",
        refused_as: "test-job-checkout",
    },
    WorkflowEscape {
        name: "MUT-WINDOWS-BUILD-WITNESS-DELETED",
        escape: "the `cargo build --all-targets` step removed. Clippy and MSRV still \
                 type-check the Windows tree on GitHub's runner, but nothing on current \
                 stable code-generates or links it: a Windows-only codegen or link failure \
                 that only current stable has passes every leg, the suite's shards among them, \
                 which link with the pinned `1.97.1` and pass.",
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
        job: Some("test-linux"),
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
        anchor: "      - run: cargo check --locked --target ${{ matrix.target }} --all-targets --all-features\n",
        replacement: "      - run: cargo check --target ${{ matrix.target }} --all-targets --all-features\n",
        refused_as: "msrv-job-command",
    },
    WorkflowEscape {
        name: "MUT-MSRV-COMMAND-ECHOED",
        escape: "the leg echoes its command and succeeds, `MUT-GATE-ECHOED`'s shape one job \
                 over. `test-docs-consistency.sh` withdrew every claim about which commands CI \
                 runs precisely because a text checker cannot tell these apart.",
        job: Some("msrv"),
        anchor: "      - run: cargo check --locked --target ${{ matrix.target }} --all-targets --all-features\n",
        replacement: "      - run: echo cargo check --locked --target ${{ matrix.target }} --all-targets --all-features\n",
        refused_as: "msrv-job-command",
    },
    WorkflowEscape {
        name: "MUT-MSRV-COMMAND-UNTARGETED",
        escape: "the check loses `--target ${{ matrix.target }}`. Each entry then checks its \
                 runner's own tuple: the macOS entry, on `ubuntu-latest`, compiles the Linux \
                 regions a second time and none of the macOS ones, and every entry is still \
                 green.",
        job: Some("msrv"),
        anchor: "      - run: cargo check --locked --target ${{ matrix.target }} --all-targets --all-features\n",
        replacement: "      - run: cargo check --locked --all-targets --all-features\n",
        refused_as: "msrv-job-command",
    },
    WorkflowEscape {
        name: "MUT-MSRV-MATRIX-NARROWED",
        escape: "the macOS entry is gone, so the floor is checked for Linux and Windows only. \
                 A dependency that raises its MSRV behind a `cfg` -- this manifest has a \
                 `cfg(windows)` and a `cfg(unix)` dependency table -- or a call newer than the \
                 floor inside a `#[cfg(target_os = \"macos\")]` region breaks on a target this \
                 leg no longer compiles.",
        job: Some("msrv"),
        anchor: "          - os: ubuntu-latest\n\
                 \x20           target: aarch64-apple-darwin\n",
        replacement: "",
        refused_as: "msrv-job-matrix",
    },
    WorkflowEscape {
        name: "MUT-MSRV-MATRIX-EXCLUDED",
        escape: "the same narrowing with `include:` left listing all three entries and an \
                 `exclude:` naming the macOS pair above it, so every reading of `include:` \
                 alone passes. `MUT-TEST-MATRIX-EXCLUDED`'s shape on this leg. GitHub \
                 documents that `include:` entries are processed after `exclude:`, so by that \
                 rule this one removes no entry of an `include:` list; that is the rule as \
                 documented, and no workflow was run to watch it. The oracle does not weigh \
                 it: the entries are read from `include:` and from nothing else in `matrix:`, \
                 so the key is refused whatever it would remove.",
        job: Some("msrv"),
        anchor: "        include:\n",
        replacement: "        exclude:\n\
                      \x20         - os: ubuntu-latest\n\
                      \x20           target: aarch64-apple-darwin\n\
                      \x20       include:\n",
        refused_as: "msrv-job-matrix",
    },
    WorkflowEscape {
        name: "MUT-MSRV-CROSS-TARGET-DROPPED",
        escape: "the macOS entry's `target:` becomes the Linux tuple. The matrix still has \
                 three entries and the job three green results, but two of them check \
                 `x86_64-unknown-linux-gnu` on `ubuntu-latest` and none compiles the macOS \
                 regions on the floor.",
        job: Some("msrv"),
        anchor: "            target: aarch64-apple-darwin\n",
        replacement: "            target: x86_64-unknown-linux-gnu\n",
        refused_as: "msrv-job-matrix",
    },
    WorkflowEscape {
        name: "MUT-MSRV-TOOLCHAIN-TARGET-DROPPED",
        escape: "the floor's toolchain step no longer installs the entry's target. The macOS \
                 entry then fails for want of that tuple's standard library, which is loud; \
                 the pin keeps the toolchain's targets and the check's `--target` reading the \
                 one matrix value.",
        job: Some("msrv"),
        anchor: "          targets: ${{ matrix.target }}\n",
        replacement: "",
        refused_as: "msrv-job-toolchain",
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
        anchor: "      - run: cargo check --locked --target ${{ matrix.target }} --all-targets --all-features\n",
        replacement: "      - run: cargo check --locked --target ${{ matrix.target }} --all-targets --all-features\n\
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
        anchor: "      - run: cargo check --locked --target ${{ matrix.target }} --all-targets --all-features\n",
        replacement: "      - run: echo \"RUSTFLAGS=-A warnings\" >> \"$GITHUB_ENV\"\n\
                      \x20     - run: cargo check --locked --target ${{ matrix.target }} --all-targets --all-features\n",
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
        job: Some("test-linux"),
        anchor: "      - run: cargo test --all-targets --all-features -- ${{ matrix.tests }}\n",
        replacement: "      - run: |\n\
                      \x20         cat >> \"$GITHUB_ENV\" <<'EOF'\n\
                      \x20         RUSTFLAGS=-A warnings\n\
                      \x20         EOF\n\
                      \x20     - run: cargo test --all-targets --all-features -- ${{ matrix.tests }}\n",
        refused_as: "rustflags-persisted",
    },
    WorkflowEscape {
        name: "MUT-RUSTFLAGS-COMMAND-SCOPED",
        escape: "the narrowing without the env file at all: the flags are scoped to one \
                 command on one line. It persists nothing, so it is the arm that proves the \
                 script scan is not merely a `GITHUB_ENV` detector -- and a leg that names the \
                 variable at all is doing something this contract has no model for.",
        job: Some("lint-macos"),
        anchor: "      - run: cargo clippy --target aarch64-apple-darwin --all-targets --all-features -- -D warnings\n",
        replacement: "      - run: RUSTFLAGS=-A warnings cargo build --all-targets\n\
                      \x20     - run: cargo clippy --target aarch64-apple-darwin --all-targets --all-features -- -D warnings\n",
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
                 \x20         targets: ${{ matrix.target }}\n\
                 \x20     - uses: Swatinem/rust-cache@6323deb102c322ba6fcbdcafc7e3dddab59af2b6 # v2.9.2\n\
                 \x20     - run: cargo check --locked --target ${{ matrix.target }} --all-targets --all-features\n",
        replacement: "      - run: cargo check --locked --target ${{ matrix.target }} --all-targets --all-features\n\
                      \x20     - uses: dtolnay/rust-toolchain@4360b52568e2003a75bf9bc1d59f33a8e3fc893c # stable\n\
                      \x20       with:\n\
                      \x20         toolchain: 1.85.0\n\
                      \x20         targets: ${{ matrix.target }}\n\
                      \x20     - uses: Swatinem/rust-cache@6323deb102c322ba6fcbdcafc7e3dddab59af2b6 # v2.9.2\n",
        refused_as: "msrv-job-order",
    },
    WorkflowEscape {
        name: "MUT-TEST-CAPTURE-STEP-DELETED",
        escape: "the hosted capture step is gone. Every leg runs its suite, reports success and \
                 leaves no host acceptance record, and nothing else here asks for one.",
        job: Some("test-linux"),
        anchor: r#"      - name: Capture the native host acceptance record
        env:
          UPSTROKE_TEST_TEMP_FOLDS_CASE: ""
        run: |
          set -o pipefail
          log="$RUNNER_TEMP/host-acceptance-record.log"
          cargo test --all-features --lib engine::tests::host_acceptance_record -- --ignored --exact --nocapture | tee "$log"
          if ! grep -qx 'test engine::tests::host_acceptance_record \.\.\. ok' "$log" || ! grep -q '^test result: ok\. 1 passed; 0 failed; 0 ignored; ' "$log"; then echo "the capture did not run exactly the one host acceptance test to a pass" >&2; exit 1; fi
          awk '
            $1 == "UPSTROKE-HOST-ACCEPTANCE-RECORD" { f[++m] = $0 }
            END {
              nb = split(f[1], b, " "); ne = split(f[m], e, " ")
              ok = m >= 3 && nb == 7 && ne == 7 && b[2] == "v1" && b[3] == "BEGIN" && e[2] == "v1" && e[3] == "END"
              ok = ok && b[4] " " b[5] " " b[6] " " b[7] == e[4] " " e[5] " " e[6] " " e[7] && b[5] "" == (m - 2) ""
              for (k = 2; ok && k < m; k++) ok = split(f[k], c, " ") == 5 && c[2] == "v1" && c[3] "" == b[4] "" && c[4] "" == (k - 1) "" && length(c[5]) == (k < m - 1 ? 192 : 2 * (b[6] - (b[5] - 1) * 96))
              exit !ok
            }' "$log" || { echo "the host acceptance record is missing or its framing is incomplete" >&2; exit 1; }
"#,
        replacement: "",
        refused_as: "test-capture-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-CAPTURE-STEP-DISABLED",
        escape: "a step-level `if: false` on the hosted capture: the job reports success with the \
                 record never made, and the suite above it still green",
        job: Some("test-macos"),
        anchor: "      - name: Capture the native host acceptance record\n",
        replacement: "      - name: Capture the native host acceptance record\n\
                      \x20       if: false\n",
        refused_as: "unexpected-step-field",
    },
    WorkflowEscape {
        name: "MUT-TEST-CAPTURE-STEP-ABSOLVED",
        escape: "`continue-on-error` on the hosted capture: a failed record, a missing frame or a \
                 test that did not pass ends the step red and the job green",
        job: Some("test-macos"),
        anchor: "      - name: Capture the native host acceptance record\n",
        replacement: "      - name: Capture the native host acceptance record\n\
                      \x20       continue-on-error: true\n",
        refused_as: "unexpected-step-field",
    },
    WorkflowEscape {
        name: "MUT-TEST-CAPTURE-IGNORED-NOT-RUN",
        escape: "`--ignored` dropped. The exact filter still names the test, libtest reports it \
                 ignored, `0 passed; 0 failed; 1 ignored`, and Cargo exits zero having run \
                 nothing: a no-op that only the pin and the step's own count refuse",
        job: Some("test-macos"),
        anchor: "          cargo test --all-features --lib engine::tests::host_acceptance_record -- --ignored --exact --nocapture | tee \"$log\"\n",
        replacement: "          cargo test --all-features --lib engine::tests::host_acceptance_record -- --exact --nocapture | tee \"$log\"\n",
        refused_as: "test-capture-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-CAPTURE-FILTER-MOVED",
        escape: "the filter names a test that does not exist, so it matches nothing: `running 0 \
                 tests`, exit zero, no record. The pin refuses the moved filter before CI runs \
                 it.",
        job: Some("test-macos"),
        anchor: "          cargo test --all-features --lib engine::tests::host_acceptance_record -- --ignored --exact --nocapture | tee \"$log\"\n",
        replacement: "          cargo test --all-features --lib engine::tests::host_acceptance_records -- --ignored --exact --nocapture | tee \"$log\"\n",
        refused_as: "test-capture-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-CAPTURE-EXIT-SWALLOWED",
        escape: "`set -o pipefail` dropped. The default `bash -e` takes the pipeline's status from \
                 `tee`, which succeeds, so a failing test or a Cargo that never built the harness \
                 ends the step on the checks after it, and a failure that still printed a pass \
                 line and a frame goes green",
        job: Some("test-macos"),
        anchor: "          set -o pipefail\n",
        replacement: "",
        refused_as: "test-capture-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-CAPTURE-ZERO-TESTS-ADMITTED",
        escape: "the line that demands the named test's pass line and `1 passed; 0 failed; 0 \
                 ignored` is gone, so a run that executed some other test, or no test at all, \
                 is judged by its frame alone",
        job: Some("test-macos"),
        anchor: "          if ! grep -qx 'test engine::tests::host_acceptance_record \\.\\.\\. ok' \"$log\" || ! grep -q '^test result: ok\\. 1 passed; 0 failed; 0 ignored; ' \"$log\"; then echo \"the capture did not run exactly the one host acceptance test to a pass\" >&2; exit 1; fi\n",
        replacement: "",
        refused_as: "test-capture-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-CAPTURE-FRAMING-UNCHECKED",
        escape: "the frame check is gone: a record cut short, a chunk lost or doubled, or no \
                 record at all passes the step, and the job is green over a record no reader \
                 can reconstruct",
        job: Some("test-macos"),
        anchor: r#"          awk '
            $1 == "UPSTROKE-HOST-ACCEPTANCE-RECORD" { f[++m] = $0 }
            END {
              nb = split(f[1], b, " "); ne = split(f[m], e, " ")
              ok = m >= 3 && nb == 7 && ne == 7 && b[2] == "v1" && b[3] == "BEGIN" && e[2] == "v1" && e[3] == "END"
              ok = ok && b[4] " " b[5] " " b[6] " " b[7] == e[4] " " e[5] " " e[6] " " e[7] && b[5] "" == (m - 2) ""
              for (k = 2; ok && k < m; k++) ok = split(f[k], c, " ") == 5 && c[2] == "v1" && c[3] "" == b[4] "" && c[4] "" == (k - 1) "" && length(c[5]) == (k < m - 1 ? 192 : 2 * (b[6] - (b[5] - 1) * 96))
              exit !ok
            }' "$log" || { echo "the host acceptance record is missing or its framing is incomplete" >&2; exit 1; }
"#,
        replacement: "",
        refused_as: "test-capture-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-CAPTURE-SEPARATED-FROM-THE-SUITE",
        escape: "a pinned cache step between the suite and the capture. Every script and action \
                 still matches its pin; the capture now runs a harness the cache may have \
                 replaced after the suite ran, so the record's binary need not be the suite's",
        job: Some("test-macos"),
        anchor: "      - name: Capture the native host acceptance record\n",
        replacement: "      - uses: Swatinem/rust-cache@6323deb102c322ba6fcbdcafc7e3dddab59af2b6 # v2.9.2\n\
                      \x20     - name: Capture the native host acceptance record\n",
        refused_as: "test-capture-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-CAPTURE-CASEFOLD-DROPPED",
        escape: "the hosted capture no longer carries its leg's case-fold declaration, so it runs \
                 under an environment the suite step did not, with its script still on its pin",
        job: Some("test-macos"),
        anchor: "      - name: Capture the native host acceptance record\n\
                 \x20       env:\n\
                 \x20         UPSTROKE_TEST_TEMP_FOLDS_CASE: \"1\"\n",
        replacement: "      - name: Capture the native host acceptance record\n",
        refused_as: "test-capture-step-env",
    },
    WorkflowEscape {
        name: "MUT-TEST-CAPTURE-RETARGETED-THROUGH-THE-ADMITTED-ENV",
        escape: "the capture's admitted `env:` gains `CARGO_TARGET_DIR`: Cargo builds and runs a \
                 harness of its own, in another target directory, rather than the one the suite \
                 step built and ran, with the script and the declaration still matching",
        job: Some("test-macos"),
        anchor: "      - name: Capture the native host acceptance record\n\
                 \x20       env:\n\
                 \x20         UPSTROKE_TEST_TEMP_FOLDS_CASE: \"1\"\n",
        replacement: "      - name: Capture the native host acceptance record\n\
                      \x20       env:\n\
                      \x20         UPSTROKE_TEST_TEMP_FOLDS_CASE: \"1\"\n\
                      \x20         CARGO_TARGET_DIR: target/capture\n",
        refused_as: "test-capture-step-env",
    },
    WorkflowEscape {
        name: "MUT-TEST-CAPABILITY-STEP-DELETED",
        escape: "the hosted observation step is gone (omitted). Every leg runs its suite and the \
                 host capture, reports success and leaves no container capability record, and \
                 nothing else here asks for one.",
        job: Some("test-macos"),
        anchor: r##"      - name: Capture the container capability observation
        timeout-minutes: 4
        env:
          UPSTROKE_TEST_TEMP_FOLDS_CASE: "1"
        run: |
          set -eo pipefail
          log="$RUNNER_TEMP/container-capability-record.log"
          cargo test --all-features --lib runner::container::tests::container_capability_record -- --ignored --exact --nocapture | tee "$log"
          if ! grep -qx 'test runner::container::tests::container_capability_record \.\.\. ok' "$log" || ! grep -q '^test result: ok\. 1 passed; 0 failed; 0 ignored; ' "$log"; then echo "the observation did not run exactly the one container capability test to a pass" >&2; exit 1; fi
          awk '
            $1 == "UPSTROKE-CONTAINER-CAPABILITY-RECORD" { f[++m] = $0 }
            END {
              nb = split(f[1], b, " "); ne = split(f[m], e, " ")
              ok = m >= 3 && nb == 7 && ne == 7 && b[2] == "v1" && b[3] == "BEGIN" && e[2] == "v1" && e[3] == "END"
              ok = ok && b[4] " " b[5] " " b[6] " " b[7] == e[4] " " e[5] " " e[6] " " e[7] && b[5] "" == (m - 2) ""
              for (k = 2; ok && k < m; k++) ok = split(f[k], c, " ") == 5 && c[2] == "v1" && c[3] "" == b[4] "" && c[4] "" == (k - 1) "" && length(c[5]) == (k < m - 1 ? 192 : 2 * (b[6] - (b[5] - 1) * 96))
              exit !ok
            }' "$log" || { echo "the container capability record is missing or its framing is incomplete" >&2; exit 1; }
"##,
        replacement: "",
        refused_as: "test-capability-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-CAPABILITY-STEP-DISABLED",
        escape: "a step-level `if: false` on the hosted observation (conditional): the job \
                 reports success with the record never made",
        job: Some("test-macos"),
        anchor: "      - name: Capture the container capability observation\n",
        replacement: "      - name: Capture the container capability observation\n\
                      \x20       if: false\n",
        refused_as: "unexpected-step-field",
    },
    WorkflowEscape {
        name: "MUT-TEST-CAPABILITY-STEP-RUN-WHEN-NOT-CANCELLED",
        escape: "the observation runs under `if: ${{ !cancelled() }}` (conditional). It then \
                 runs after a failed suite or capture, so its record no longer stands on a \
                 harness the suite passed, and the contract places it success-only",
        job: Some("test-macos"),
        anchor: "      - name: Capture the container capability observation\n",
        replacement: "      - name: Capture the container capability observation\n\
                      \x20       if: ${{ !cancelled() }}\n",
        refused_as: "unexpected-step-field",
    },
    WorkflowEscape {
        name: "MUT-TEST-CAPABILITY-STEP-ABSOLVED",
        escape: "`continue-on-error` on the hosted observation (failure-swallowing): an unresolved \
                 process, a refused record or a missing frame ends the step red and the job green",
        job: Some("test-macos"),
        anchor: "      - name: Capture the container capability observation\n",
        replacement: "      - name: Capture the container capability observation\n\
                      \x20       continue-on-error: true\n",
        refused_as: "unexpected-step-field",
    },
    WorkflowEscape {
        name: "MUT-TEST-CAPABILITY-IGNORED-NOT-RUN",
        escape: "`--ignored` dropped (no-op): libtest reports the test ignored, `0 passed; 0 \
                 failed; 1 ignored`, and Cargo exits zero having asked nothing",
        job: Some("test-macos"),
        anchor: "          cargo test --all-features --lib runner::container::tests::container_capability_record -- --ignored --exact --nocapture | tee \"$log\"\n",
        replacement: "          cargo test --all-features --lib runner::container::tests::container_capability_record -- --exact --nocapture | tee \"$log\"\n",
        refused_as: "test-capability-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-CAPABILITY-FILTER-MOVED",
        escape: "the filter names a test that does not exist (filtered): `running 0 tests`, exit \
                 zero, no record",
        job: Some("test-macos"),
        anchor: "          cargo test --all-features --lib runner::container::tests::container_capability_record -- --ignored --exact --nocapture | tee \"$log\"\n",
        replacement: "          cargo test --all-features --lib runner::container::tests::container_capability_records -- --ignored --exact --nocapture | tee \"$log\"\n",
        refused_as: "test-capability-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-CAPABILITY-FILTER-WIDENED",
        escape: "`--exact` dropped (filtered): the name becomes a substring filter, so any ignored \
                 test whose path contains it runs beside the observation",
        job: Some("test-macos"),
        anchor: "          cargo test --all-features --lib runner::container::tests::container_capability_record -- --ignored --exact --nocapture | tee \"$log\"\n",
        replacement: "          cargo test --all-features --lib runner::container::tests::container_capability_record -- --ignored --nocapture | tee \"$log\"\n",
        refused_as: "test-capability-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-CAPABILITY-ECHOED",
        escape: "the Cargo line is echoed (no-op): the step prints the command, asks nothing, and \
                 only its own pass-line check would notice",
        job: Some("test-macos"),
        anchor: "          cargo test --all-features --lib runner::container::tests::container_capability_record -- --ignored --exact --nocapture | tee \"$log\"\n",
        replacement: "          echo cargo test --all-features --lib runner::container::tests::container_capability_record -- --ignored --exact --nocapture | tee \"$log\"\n",
        refused_as: "test-capability-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-CAPABILITY-EXIT-SWALLOWED",
        escape: "`set -eo pipefail` dropped (failure-swallowing). Under the default `bash -e` the \
                 pipeline takes its status from `tee`, so a failing observation that still \
                 printed a pass line and a frame goes green",
        job: Some("test-macos"),
        anchor: "          set -eo pipefail\n",
        replacement: "",
        refused_as: "test-capability-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-CAPABILITY-ZERO-TESTS-ADMITTED",
        escape: "the pass-line and `1 passed; 0 failed; 0 ignored` check is gone, so a run of no \
                 test or of another is judged by its frame alone",
        job: Some("test-macos"),
        anchor: "          if ! grep -qx 'test runner::container::tests::container_capability_record \\.\\.\\. ok' \"$log\" || ! grep -q '^test result: ok\\. 1 passed; 0 failed; 0 ignored; ' \"$log\"; then echo \"the observation did not run exactly the one container capability test to a pass\" >&2; exit 1; fi\n",
        replacement: "",
        refused_as: "test-capability-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-CAPABILITY-FRAMING-UNCHECKED",
        escape: "the frame check is gone: a record cut short, a chunk lost or doubled, or no \
                 record at all passes the step",
        job: Some("test-macos"),
        anchor: r##"          awk '
            $1 == "UPSTROKE-CONTAINER-CAPABILITY-RECORD" { f[++m] = $0 }
            END {
              nb = split(f[1], b, " "); ne = split(f[m], e, " ")
              ok = m >= 3 && nb == 7 && ne == 7 && b[2] == "v1" && b[3] == "BEGIN" && e[2] == "v1" && e[3] == "END"
              ok = ok && b[4] " " b[5] " " b[6] " " b[7] == e[4] " " e[5] " " e[6] " " e[7] && b[5] "" == (m - 2) ""
              for (k = 2; ok && k < m; k++) ok = split(f[k], c, " ") == 5 && c[2] == "v1" && c[3] "" == b[4] "" && c[4] "" == (k - 1) "" && length(c[5]) == (k < m - 1 ? 192 : 2 * (b[6] - (b[5] - 1) * 96))
              exit !ok
            }' "$log" || { echo "the container capability record is missing or its framing is incomplete" >&2; exit 1; }
"##,
        replacement: "",
        refused_as: "test-capability-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-CAPABILITY-SEPARATED-FROM-THE-CAPTURE",
        escape: "a pinned cache step between the host capture and the observation (misplaced): \
                 every script and action still matches its pin, and the observation runs a \
                 harness the cache may have replaced after the suite ran",
        job: Some("test-macos"),
        anchor: "      - name: Capture the container capability observation\n",
        replacement: "      - uses: Swatinem/rust-cache@6323deb102c322ba6fcbdcafc7e3dddab59af2b6 # v2.9.2\n\
                      \x20     - name: Capture the container capability observation\n",
        refused_as: "test-capability-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-CAPABILITY-ABOVE-THE-CAPTURE",
        escape: "the observation moved above the host capture (misplaced): both scripts still \
                 match their pins and each runs once, but the observation no longer follows \
                 the record the capture makes",
        job: Some("test-macos"),
        anchor: r##"      - name: Capture the native host acceptance record
        env:
          UPSTROKE_TEST_TEMP_FOLDS_CASE: "1"
        run: |
          set -o pipefail
          log="$RUNNER_TEMP/host-acceptance-record.log"
          cargo test --all-features --lib engine::tests::host_acceptance_record -- --ignored --exact --nocapture | tee "$log"
          if ! grep -qx 'test engine::tests::host_acceptance_record \.\.\. ok' "$log" || ! grep -q '^test result: ok\. 1 passed; 0 failed; 0 ignored; ' "$log"; then echo "the capture did not run exactly the one host acceptance test to a pass" >&2; exit 1; fi
          awk '
            $1 == "UPSTROKE-HOST-ACCEPTANCE-RECORD" { f[++m] = $0 }
            END {
              nb = split(f[1], b, " "); ne = split(f[m], e, " ")
              ok = m >= 3 && nb == 7 && ne == 7 && b[2] == "v1" && b[3] == "BEGIN" && e[2] == "v1" && e[3] == "END"
              ok = ok && b[4] " " b[5] " " b[6] " " b[7] == e[4] " " e[5] " " e[6] " " e[7] && b[5] "" == (m - 2) ""
              for (k = 2; ok && k < m; k++) ok = split(f[k], c, " ") == 5 && c[2] == "v1" && c[3] "" == b[4] "" && c[4] "" == (k - 1) "" && length(c[5]) == (k < m - 1 ? 192 : 2 * (b[6] - (b[5] - 1) * 96))
              exit !ok
            }' "$log" || { echo "the host acceptance record is missing or its framing is incomplete" >&2; exit 1; }

      # The container capability observation for this platform (Gate7 M2's
      # runtime question, contract version 2 under its root audit).
      # `runner::container::tests::container_capability_record` is ignored by
      # the suite and selected here by its exact name. It asks the CLI the
      # contract's bounded read-only questions -- the endpoint the CLI
      # resolved, the daemon's version, selected version fields and three
      # local image inspections, never a pull -- through the product's
      # process path, reduces every endpoint-sensitive answer in memory, and
      # prints the reduced record as numbered hex lines between BEGIN and END
      # (`docs/internals/runner/container/tests.md`). Every capability class
      # passes, an absent Docker included; an unresolved process, a binding
      # failure or a record its own decoder refuses fails the test.
      # `pipefail` carries Cargo's exit through `tee`; the step then refuses
      # unless exactly that test ran and passed, and refuses a missing or
      # incomplete frame. A class is an observation, not M2 acceptance, and
      # the Linux row is a control. Four minutes bound the 135 s the
      # questions may request, with Cargo's start. The step adds no
      # environment of its own; the workflow oracle pins it whole,
      # immediately after the host acceptance capture.
      - name: Capture the container capability observation
        timeout-minutes: 4
        env:
          UPSTROKE_TEST_TEMP_FOLDS_CASE: "1"
        run: |
          set -eo pipefail
          log="$RUNNER_TEMP/container-capability-record.log"
          cargo test --all-features --lib runner::container::tests::container_capability_record -- --ignored --exact --nocapture | tee "$log"
          if ! grep -qx 'test runner::container::tests::container_capability_record \.\.\. ok' "$log" || ! grep -q '^test result: ok\. 1 passed; 0 failed; 0 ignored; ' "$log"; then echo "the observation did not run exactly the one container capability test to a pass" >&2; exit 1; fi
          awk '
            $1 == "UPSTROKE-CONTAINER-CAPABILITY-RECORD" { f[++m] = $0 }
            END {
              nb = split(f[1], b, " "); ne = split(f[m], e, " ")
              ok = m >= 3 && nb == 7 && ne == 7 && b[2] == "v1" && b[3] == "BEGIN" && e[2] == "v1" && e[3] == "END"
              ok = ok && b[4] " " b[5] " " b[6] " " b[7] == e[4] " " e[5] " " e[6] " " e[7] && b[5] "" == (m - 2) ""
              for (k = 2; ok && k < m; k++) ok = split(f[k], c, " ") == 5 && c[2] == "v1" && c[3] "" == b[4] "" && c[4] "" == (k - 1) "" && length(c[5]) == (k < m - 1 ? 192 : 2 * (b[6] - (b[5] - 1) * 96))
              exit !ok
            }' "$log" || { echo "the container capability record is missing or its framing is incomplete" >&2; exit 1; }
"##,
        replacement: r##"      - name: Capture the container capability observation
        timeout-minutes: 4
        env:
          UPSTROKE_TEST_TEMP_FOLDS_CASE: "1"
        run: |
          set -eo pipefail
          log="$RUNNER_TEMP/container-capability-record.log"
          cargo test --all-features --lib runner::container::tests::container_capability_record -- --ignored --exact --nocapture | tee "$log"
          if ! grep -qx 'test runner::container::tests::container_capability_record \.\.\. ok' "$log" || ! grep -q '^test result: ok\. 1 passed; 0 failed; 0 ignored; ' "$log"; then echo "the observation did not run exactly the one container capability test to a pass" >&2; exit 1; fi
          awk '
            $1 == "UPSTROKE-CONTAINER-CAPABILITY-RECORD" { f[++m] = $0 }
            END {
              nb = split(f[1], b, " "); ne = split(f[m], e, " ")
              ok = m >= 3 && nb == 7 && ne == 7 && b[2] == "v1" && b[3] == "BEGIN" && e[2] == "v1" && e[3] == "END"
              ok = ok && b[4] " " b[5] " " b[6] " " b[7] == e[4] " " e[5] " " e[6] " " e[7] && b[5] "" == (m - 2) ""
              for (k = 2; ok && k < m; k++) ok = split(f[k], c, " ") == 5 && c[2] == "v1" && c[3] "" == b[4] "" && c[4] "" == (k - 1) "" && length(c[5]) == (k < m - 1 ? 192 : 2 * (b[6] - (b[5] - 1) * 96))
              exit !ok
            }' "$log" || { echo "the container capability record is missing or its framing is incomplete" >&2; exit 1; }
      - name: Capture the native host acceptance record
        env:
          UPSTROKE_TEST_TEMP_FOLDS_CASE: "1"
        run: |
          set -o pipefail
          log="$RUNNER_TEMP/host-acceptance-record.log"
          cargo test --all-features --lib engine::tests::host_acceptance_record -- --ignored --exact --nocapture | tee "$log"
          if ! grep -qx 'test engine::tests::host_acceptance_record \.\.\. ok' "$log" || ! grep -q '^test result: ok\. 1 passed; 0 failed; 0 ignored; ' "$log"; then echo "the capture did not run exactly the one host acceptance test to a pass" >&2; exit 1; fi
          awk '
            $1 == "UPSTROKE-HOST-ACCEPTANCE-RECORD" { f[++m] = $0 }
            END {
              nb = split(f[1], b, " "); ne = split(f[m], e, " ")
              ok = m >= 3 && nb == 7 && ne == 7 && b[2] == "v1" && b[3] == "BEGIN" && e[2] == "v1" && e[3] == "END"
              ok = ok && b[4] " " b[5] " " b[6] " " b[7] == e[4] " " e[5] " " e[6] " " e[7] && b[5] "" == (m - 2) ""
              for (k = 2; ok && k < m; k++) ok = split(f[k], c, " ") == 5 && c[2] == "v1" && c[3] "" == b[4] "" && c[4] "" == (k - 1) "" && length(c[5]) == (k < m - 1 ? 192 : 2 * (b[6] - (b[5] - 1) * 96))
              exit !ok
            }' "$log" || { echo "the host acceptance record is missing or its framing is incomplete" >&2; exit 1; }
"##,
        refused_as: "test-capability-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-CAPABILITY-TIMEOUT-DROPPED",
        escape: "the observation's 4-minute bound is gone: a stalled question is held by the \
                 job's own timeout, past the step's budget",
        job: Some("test-macos"),
        anchor: "        timeout-minutes: 4\n",
        replacement: "",
        refused_as: "test-capability-step",
    },
    WorkflowEscape {
        name: "MUT-TEST-CAPABILITY-TIMEOUT-WIDENED",
        escape: "the observation's bound widened to 30 minutes: every field still present, and a \
                 stalled question spends the leg's headroom",
        job: Some("test-macos"),
        anchor: "        timeout-minutes: 4\n",
        replacement: "        timeout-minutes: 30\n",
        refused_as: "test-capability-step",
    },
    WorkflowEscape {
        name: "MUT-TEST-CAPABILITY-DOCKER-HOST-INJECTED",
        escape: "the observation's admitted `env:` gains `DOCKER_HOST`: the step no longer \
                 observes the leg's own environment but one this file chose, with its script \
                 still on its pin",
        job: Some("test-macos"),
        anchor: "      - name: Capture the container capability observation\n\
                 \x20       timeout-minutes: 4\n\
                 \x20       env:\n\
                 \x20         UPSTROKE_TEST_TEMP_FOLDS_CASE: \"1\"\n",
        replacement: "      - name: Capture the container capability observation\n\
                      \x20       timeout-minutes: 4\n\
                      \x20       env:\n\
                      \x20         UPSTROKE_TEST_TEMP_FOLDS_CASE: \"1\"\n\
                      \x20         DOCKER_HOST: tcp://127.0.0.1:2375\n",
        refused_as: "test-capability-step-env",
    },
    WorkflowEscape {
        name: "MUT-TEST-CAPABILITY-REQUIRE-DOCKER-INJECTED",
        escape: "the observation's admitted `env:` gains `UPSTROKE_REQUIRE_DOCKER`: the contract \
                 runs it in the leg's environment unchanged, and the variable is the suite's \
                 Docker requirement, not this record's",
        job: Some("test-macos"),
        anchor: "      - name: Capture the container capability observation\n\
                 \x20       timeout-minutes: 4\n\
                 \x20       env:\n\
                 \x20         UPSTROKE_TEST_TEMP_FOLDS_CASE: \"1\"\n",
        replacement: "      - name: Capture the container capability observation\n\
                      \x20       timeout-minutes: 4\n\
                      \x20       env:\n\
                      \x20         UPSTROKE_TEST_TEMP_FOLDS_CASE: \"1\"\n\
                      \x20         UPSTROKE_REQUIRE_DOCKER: \"1\"\n",
        refused_as: "test-capability-step-env",
    },
    WorkflowEscape {
        name: "MUT-TEST-CAPABILITY-CASEFOLD-DROPPED",
        escape: "the hosted observation no longer carries its leg's case-fold declaration, so it \
                 runs under an environment the suite step did not",
        job: Some("test-macos"),
        anchor: "        timeout-minutes: 4\n\
                 \x20       env:\n\
                 \x20         UPSTROKE_TEST_TEMP_FOLDS_CASE: \"1\"\n",
        replacement: "        timeout-minutes: 4\n",
        refused_as: "test-capability-step-env",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-WITNESS-DROPPED",
        escape: "the shards' suite step keeps the suite command and loses the count: the line \
                 that sums it, the line that holds it to the shard's floor and the line that \
                 prints it. A Cargo target runner in the runner's environment or Cargo home \
                 then hands every harness to a wrapper that exits zero, and each of the fourteen \
                 legs is green having executed nothing, on every event: no other job runs the \
                 Windows suite",
        job: Some("test-windows-hosted"),
        anchor: "          $passed = [int](($log | Select-String -Pattern '^test result: ok\\. (\\d+) passed' | ForEach-Object { [int]$_.Matches[0].Groups[1].Value } | Measure-Object -Sum).Sum)\n          if ($passed -lt ${{ matrix.floor }}) { throw \"shard ${{ matrix.shard }} reported $passed passing tests, below its floor of ${{ matrix.floor }}: Cargo compiled the harnesses and executed almost none of this shard's tests\" }\n          \"shard ${{ matrix.shard }} reported $passed passing tests; its floor is ${{ matrix.floor }}\"\n",
        replacement: "",
        refused_as: "test-windows-hosted-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-FLOOR-DROPPED",
        escape: "the shards' count stays and the floor it is held to drops from the shard's to \
                 zero, so a shard that executed no test clears it, on every leg at once and \
                 with the table of floors untouched: the matrix still hands each leg its floor, \
                 and the step holds no leg to it",
        job: Some("test-windows-hosted"),
        anchor: "          if ($passed -lt ${{ matrix.floor }}) { throw \"shard ${{ matrix.shard }} reported $passed passing tests, below its floor of ${{ matrix.floor }}: Cargo compiled the harnesses and executed almost none of this shard's tests\" }\n",
        replacement: "          if ($passed -lt 0) { throw \"shard ${{ matrix.shard }} reported $passed passing tests, below its floor of ${{ matrix.floor }}: Cargo compiled the harnesses and executed almost none of this shard's tests\" }\n",
        refused_as: "test-windows-hosted-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-COMPILER-CHECK-DROPPED",
        escape: "the suite step's two compiler lines dropped, the install left at the pinned \
                 `1.97.1`. The toolchain action tolerates a failed `rustup default` and verifies \
                 only `rustc +1.97.1`, so the install can report success while bare `cargo` is \
                 the runner's preinstalled stable, and a `RUSTC` or a PATH naming another \
                 install is read by nothing else here: every shard runs its tests on a compiler \
                 this workflow did not choose, with every other pin matching",
        job: Some("test-windows-hosted"),
        anchor: "          $rustc = ((cargo rustc -q --lib -- --version) | Out-String).Trim(); $path_rustc = ((rustc --version) | Out-String).Trim(); $cargo = ((cargo --version) | Out-String).Trim()\n\
                 \x20         if ($rustc -notmatch '^rustc 1\\.97\\.1 ' -or $path_rustc -notmatch '^rustc 1\\.97\\.1 ' -or $cargo -notmatch '^cargo 1\\.97\\.1 ') { throw \"the suite would run on '$rustc' (Cargo's compiler), '$path_rustc' (PATH's rustc) and '$cargo', not the image's 1.97.1: a compiler was chosen by something other than this workflow\" }\n",
        replacement: "",
        refused_as: "test-windows-hosted-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-TIMEOUT-RAISED",
        escape: "a shard's ceiling raised from its 40 minutes, which is 1.75 times the slowest \
                 leg that ended in run 43719719561 and inside the merge queue's 60, to two \
                 hours. A leg that prints every result line and does not end, as one did in \
                 that run, where a leftover test process is inferred to have held the step's \
                 output, then holds the required check and the queue entry behind it until \
                 that ceiling",
        job: Some("test-windows-hosted"),
        anchor: "    timeout-minutes: 40\n",
        replacement: "    timeout-minutes: 120\n",
        refused_as: "test-windows-hosted-timeout",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-ON-THE-GUEST",
        escape: "the fourteen legs moved onto the labels of a self-hosted guest deleted on \
                 2026-11-20, which no runner carries: every leg of every run is queued for a \
                 machine that does not exist, and the Windows suite starts on no event",
        job: Some("test-windows-hosted"),
        anchor: "    runs-on: windows-latest\n",
        replacement: "    runs-on: [self-hosted, windows, winguest]\n",
        refused_as: "test-windows-hosted-runner",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-TOOLCHAIN-DOWNGRADED",
        escape: "the shards install `1.96.0`, below the pinned `1.97.1`. The action, its input \
                 keys and its components value are allowlisted, so every step-level pin holds. \
                 With the suite step intact its compiler questions throw on every leg of every \
                 run; edited together with it, the suite omits a Windows test enabled only on \
                 1.97 and later, on every event, and no run says so",
        job: Some("test-windows-hosted"),
        anchor: "          toolchain: 1.97.1\n",
        replacement: "          toolchain: 1.96.0\n",
        refused_as: "test-windows-hosted-toolchain",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-CACHE-RESTORED",
        escape: "the cache step this job first had, put back between the install and the \
                 identity step. The action is pinned and given no input, so every other pin \
                 holds. Every push to `master` now runs the shards, so `master` would save a \
                 cache that pull requests and queue entries could restore; whether one pays \
                 inside a leg's 40 minutes is not measured, and the oracle refuses the step \
                 until a change measures it",
        job: Some("test-windows-hosted"),
        anchor: "      - name: Configure git identity\n",
        replacement: "      - name: Cache cargo\n\
                      \x20       uses: Swatinem/rust-cache@6323deb102c322ba6fcbdcafc7e3dddab59af2b6 # v2.9.2\n\n\
                      \x20     - name: Configure git identity\n",
        refused_as: "test-windows-hosted-cache",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-TOOLCHAIN-FLOATS",
        escape: "the shards install `stable`, the shape this job first had, which is already \
                 ahead of the pinned `1.97.1`. With the suite step intact its compiler \
                 questions throw on every leg of every run; edited together with it, the \
                 suite's compiler moves with GitHub's stable and not by an edit here, and a \
                 test whose outcome depends on the compiler -- a std change to Windows process \
                 spawning, a new lint in test code under `-D warnings` -- passes one week and \
                 fails the next on an unchanged tree. While a guest also ran the suite this was \
                 a compiler split between the suite's two homes, which the hosted merge-queue \
                 lane's pin had removed and the first review of #375 found this job had brought \
                 back",
        job: Some("test-windows-hosted"),
        anchor: "          toolchain: 1.97.1\n",
        replacement: "          toolchain: stable\n",
        refused_as: "test-windows-hosted-toolchain",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-WITHOUT-CLIPPY",
        escape: "the shards install the pinned `1.97.1` without `clippy`, so the effect-denial \
                 fixtures in the suite they run have no `clippy-driver`",
        job: Some("test-windows-hosted"),
        anchor: "          components: clippy\n",
        replacement: "",
        refused_as: "test-windows-hosted-toolchain",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-CASEFOLD-DECLARATION-DROPPED",
        escape: "the shards' suite step no longer declares that the runner's NTFS folds case, \
                 so the alias test observes its native branch there instead of requiring it, on \
                 the one job that runs the Windows suite",
        job: Some("test-windows-hosted"),
        anchor: "      - name: Test, and witness that the suite ran\n\
                 \x20       env:\n\
                 \x20         UPSTROKE_TEST_TEMP_FOLDS_CASE: \"1\"\n",
        replacement: "      - name: Test, and witness that the suite ran\n",
        refused_as: "test-windows-hosted-step-env",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-CAPTURE-SEPARATED-FROM-THE-WITNESS",
        escape: "a pinned cache step between the shards' suite step and their capture: the \
                 record need not come from the harness the witness counted",
        job: Some("test-windows-hosted"),
        anchor: "      - name: Capture the native host acceptance record\n",
        replacement: "      - uses: Swatinem/rust-cache@6323deb102c322ba6fcbdcafc7e3dddab59af2b6 # v2.9.2\n\
                      \x20     - name: Capture the native host acceptance record\n",
        refused_as: "test-windows-hosted-capture-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-CHECKOUT-REF",
        escape: "the shards check out `master`: on every event the fourteen legs test a tree the \
                 candidate never touched, and the Windows suite is green over a candidate it \
                 never ran",
        job: Some("test-windows-hosted"),
        anchor: "      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0\n",
        replacement: "      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0\n\
                      \x20       with:\n\
                      \x20         ref: master\n",
        refused_as: "test-windows-hosted-checkout",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-SHARD-NARROWED",
        escape: "the last shard, the one with no filter, gains a `--skip effects::`. Every test \
                 of that shard whose name contains `effects::` then runs in no leg, this \
                 oracle's among them. The leg's floor is about half of its count, so where the \
                 skip takes less than the other half all fourteen legs are green over a suite \
                 that lost those tests",
        job: Some("test-windows-hosted"),
        anchor: "          - shard: \"14 of 14\"\n            tests: \"",
        replacement: "          - shard: \"14 of 14\"\n            tests: \"--skip effects:: ",
        refused_as: "test-windows-hosted-matrix",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-LAST-SHARD-SKIPS-MORE",
        escape: "the same skip put last in the last shard's arguments, after the 69 the table \
                 derives. Every test of that shard whose name contains `effects::` runs in no \
                 leg, as under `MUT-TEST-WINDOWS-HOSTED-SHARD-NARROWED`; here the string opens \
                 as the table's does and every word of the table's is in its place, so a \
                 reading that stopped at the table's last word would pass it",
        job: Some("test-windows-hosted"),
        anchor: "--skip engine::tests::f\"\n            floor: 985\n",
        replacement: "--skip engine::tests::f --skip effects::\"\n            floor: 985\n",
        refused_as: "test-windows-hosted-matrix",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-SHARD-DROPPED",
        escape: "the second shard's entry gone from the `include:` list, its three lines. Its \
                 tests, `runner::contract::`'s among them, are named by none of the first \
                 shard's filters: every later leg skips them and no leg left asks for them, so \
                 they run on no hosted leg. The thirteen legs left each clear their own floor, \
                 the job's result is `success`, and the aggregate reads a Windows suite that \
                 passed",
        job: Some("test-windows-hosted"),
        anchor: "          - shard: \"2 of 14\"\n\
                 \x20           tests: \"effects::tests::every_pr6_refusal_st16_variant_and_invariant_clause_names_a_test_or_an_owner engine::topology::recover::tests::a_k engine::topology::recover::tests::an engine::tests::a_p runner::contract:: engine::topology::coordinator::tests::o --skip engine::topology::coordinator::tests::a_h --skip engine::topology::coordinator::tests::t --skip engine::topology::recover::tests::k --skip workspace_manager::tests::c --skip engine::tests::a_t\"\n\
                 \x20           floor: 44\n",
        replacement: "",
        refused_as: "test-windows-hosted-matrix",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-FILTER-DROPPED-FROM-ITS-LEG",
        escape: "the second shard's first filter, \
                 `effects::tests::every_pr6_refusal_st16_variant_and_invariant_clause_names_a_test_or_an_owner`, \
                 deleted from that shard's own `tests:` line and from nowhere else. Every later \
                 shard still skips it and the first never named it, so the one test it names \
                 runs on no hosted leg, while the other thirteen legs run what the table gives \
                 them. The leg's floor counts tests, not which ones, so it refuses the loss \
                 only where the leg's count was within one of it",
        job: Some("test-windows-hosted"),
        anchor: "            tests: \"effects::tests::every_pr6_refusal_st16_variant_and_invariant_clause_names_a_test_or_an_owner engine::topology::recover::tests::a_k ",
        replacement: "            tests: \"engine::topology::recover::tests::a_k ",
        refused_as: "test-windows-hosted-matrix",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-SKIP-DROPPED",
        escape: "the last shard's first skip, `--skip engine::topology::coordinator::tests::a_h`, \
                 deleted. The first shard runs the coordinator's tests whose names open `a_h`, \
                 and the last no longer excludes them, so they run twice, in the first leg and \
                 again in the last. A count only grows by it, so no floor refuses it, and the \
                 last leg spends those tests' time a second time",
        job: Some("test-windows-hosted"),
        anchor: "          - shard: \"14 of 14\"\n            tests: \"--skip engine::topology::coordinator::tests::a_h --skip ",
        replacement: "          - shard: \"14 of 14\"\n            tests: \"--skip ",
        refused_as: "test-windows-hosted-matrix",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-FILTER-MOVED-BETWEEN-LEGS",
        escape: "the third shard's last filter, `engine::tests::a_s`, moved to the front of the \
                 fourth shard's filters: one word moved and none lost, so every filter of the \
                 table is still in the matrix. The fourth shard still skips it, as every shard \
                 after the third does, and libtest runs no test a `--skip` names whatever filter \
                 selects it, so the engine's own tests whose names open `a_s` run on no hosted \
                 leg. The two legs' floors count tests, not which ones",
        job: Some("test-windows-hosted"),
        anchor: " engine::tests::a_s --skip engine::topology::coordinator::tests::a_h --skip engine::topology::coordinator::tests::t --skip engine::topology::recover::tests::k --skip workspace_manager::tests::c --skip engine::tests::a_t --skip effects::tests::every_pr6_refusal_st16_variant_and_invariant_clause_names_a_test_or_an_owner --skip engine::topology::recover::tests::a_k --skip engine::topology::recover::tests::an --skip engine::tests::a_p --skip runner::contract:: --skip engine::topology::coordinator::tests::o\"\n            floor: 224\n          - shard: \"4 of 14\"\n            tests: \"",
        replacement: " --skip engine::topology::coordinator::tests::a_h --skip engine::topology::coordinator::tests::t --skip engine::topology::recover::tests::k --skip workspace_manager::tests::c --skip engine::tests::a_t --skip effects::tests::every_pr6_refusal_st16_variant_and_invariant_clause_names_a_test_or_an_owner --skip engine::topology::recover::tests::a_k --skip engine::topology::recover::tests::an --skip engine::tests::a_p --skip runner::contract:: --skip engine::topology::coordinator::tests::o\"\n            floor: 224\n          - shard: \"4 of 14\"\n            tests: \"engine::tests::a_s ",
        refused_as: "test-windows-hosted-matrix",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-FILTER-WIDENED",
        escape: "the first shard's `engine::topology::coordinator::tests::a_h`, a filter that \
                 opens a test's name, cut by its last character to \
                 `engine::topology::coordinator::tests::a_` on that shard's line alone. The \
                 first leg then runs every coordinator test whose name opens `a_`, and every \
                 later leg skips only the `a_h` it still names, so the ones a later leg runs -- \
                 the fourth's `a_p`, the tenth's `a_r` -- run there too: twice. A count only \
                 grows by it, so no floor refuses it",
        job: Some("test-windows-hosted"),
        anchor: "            tests: \"engine::topology::coordinator::tests::a_h engine::",
        replacement: "            tests: \"engine::topology::coordinator::tests::a_ engine::",
        refused_as: "test-windows-hosted-matrix",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-FILTERS-COMMA-JOINED",
        escape: "the first shard's first two filters joined by a comma, \
                 `engine::topology::coordinator::tests::a_h,engine::topology::coordinator::tests::t`. \
                 libtest takes no list: the word is one filter, a substring no test's name \
                 contains, so the first leg runs neither the coordinator's tests whose names \
                 open `a_h` nor those that open `t`, and every later leg still skips both: they \
                 run on no hosted leg. The first leg's floor counts tests, not which ones",
        job: Some("test-windows-hosted"),
        anchor: "            tests: \"engine::topology::coordinator::tests::a_h engine::topology::coordinator::tests::t ",
        replacement: "            tests: \"engine::topology::coordinator::tests::a_h,engine::topology::coordinator::tests::t ",
        refused_as: "test-windows-hosted-matrix",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-SHARD-FLOOR-LOWERED",
        escape: "the last shard's floor lowered to 1 in the table, the script untouched. The \
                 step still tests the count against `matrix.floor`, so its pin holds, and a leg \
                 that executed one test clears the floor the matrix now hands it",
        job: Some("test-windows-hosted"),
        anchor: "            floor: 985\n",
        replacement: "            floor: 1\n",
        refused_as: "test-windows-hosted-matrix",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-SHARD-FLOOR-ZEROED",
        escape: "the last shard's floor set to zero in the table, the script untouched. It is \
                 the largest of the fourteen, 985 of 1700, the floor the whole suite was held \
                 to as one job, and with it gone the leg is green having executed nothing: the \
                 count the step sums is never below zero. \
                 `MUT-TEST-WINDOWS-HOSTED-SHARD-FLOOR-LOWERED`'s floor of 1 still asks for one \
                 test; this asks for none, which is the arrival the count exists to refuse",
        job: Some("test-windows-hosted"),
        anchor: "            floor: 985\n",
        replacement: "            floor: 0\n",
        refused_as: "test-windows-hosted-matrix",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-FAIL-FAST-ENABLED",
        escape: "`fail-fast: true` on the shard matrix: the first leg to fail cancels the legs \
                 still running, so a run reports one shard's failing tests and none of the \
                 others', and the run that repairs them finds the next shard's",
        job: Some("test-windows-hosted"),
        anchor: "      fail-fast: false\n",
        replacement: "      fail-fast: true\n",
        refused_as: "test-windows-hosted-matrix",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-MATRIX-AXIS-ADDED",
        escape: "an axis beside the `include:` list, which still names all fourteen shards. By \
                 GitHub's documented rule for `include:`, each entry is added to the axis's one \
                 combination instead of making a leg of its own, and a later entry overwrites \
                 the `shard`, `tests` and `floor` an earlier one added: one leg runs, the last \
                 shard's, it clears its own floor, the job's result is `success`, and thirteen \
                 shards' tests ran nowhere",
        job: Some("test-windows-hosted"),
        anchor: "      matrix:\n        include:\n",
        replacement: "      matrix:\n        os: [windows-latest]\n        include:\n",
        refused_as: "test-windows-hosted-matrix",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-SHARD-EXCLUDED",
        escape: "an `exclude:` naming the second shard, beside the `include:` list that still \
                 names all fourteen. Were it to remove that leg, the shard's tests would run on no \
                 hosted leg while every reading of the list passed. GitHub documents that \
                 `include:` entries are processed after `exclude:`, so by that rule it removes \
                 no leg of a list with no axis; that is the rule as documented, and no workflow \
                 was run to watch it. The oracle does not weigh it: the legs are read from \
                 `include:` and from nothing else in `matrix:`, so the key is refused whatever \
                 it would remove",
        job: Some("test-windows-hosted"),
        anchor: "        include:\n",
        replacement: "        exclude:\n\
                      \x20         - shard: \"2 of 14\"\n\
                      \x20       include:\n",
        refused_as: "test-windows-hosted-matrix",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-SHARD-SELECTION-DROPPED",
        escape: "the line that runs the harnesses loses the leg's selection, so each harness runs \
                 whole again, the table and the floors untouched: each of the fourteen legs runs \
                 the whole suite. Measured on 2026-11-15 at 59 to 76 minutes a job, that is \
                 fourteen legs cancelled at their limit; on a faster runner it is fourteen green \
                 legs and no shard",
        job: Some("test-windows-hosted"),
        anchor: "          $env:RUSTC_BOOTSTRAP = '1'; foreach ($h in $harnesses) { $suite = Start-Process -FilePath $h -ArgumentList '${{ matrix.tests }} -Z unstable-options --report-time' -NoNewWindow -PassThru -RedirectStandardOutput \"$out.part\" -RedirectStandardError \"$err.part\"; $null = $suite.Handle; $suite.WaitForExit(); $code = $suite.ExitCode; try { Get-Content \"$out.part\" | Add-Content $out } catch { \"copying the output of $h failed: $_\"; if ($code -eq 0) { throw } }; try { Get-Content \"$err.part\" | Add-Content $err } catch { \"copying the errors of $h failed: $_\"; if ($code -eq 0) { throw } }; if ($code -ne 0) { break } }\n",
        replacement: "          $env:RUSTC_BOOTSTRAP = '1'; foreach ($h in $harnesses) { $suite = Start-Process -FilePath $h -ArgumentList '-Z unstable-options --report-time' -NoNewWindow -PassThru -RedirectStandardOutput \"$out.part\" -RedirectStandardError \"$err.part\"; $null = $suite.Handle; $suite.WaitForExit(); $code = $suite.ExitCode; try { Get-Content \"$out.part\" | Add-Content $out } catch { \"copying the output of $h failed: $_\"; if ($code -eq 0) { throw } }; try { Get-Content \"$err.part\" | Add-Content $err } catch { \"copying the errors of $h failed: $_\"; if ($code -eq 0) { throw } }; if ($code -ne 0) { break } }\n",
        refused_as: "test-windows-hosted-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-FILTER-HARDCODED",
        escape: "the line that runs the harnesses names one filter, `engine::`, where it read \
                 the leg's `matrix.tests`, the table and the floors untouched. Every leg then \
                 runs the one subset, and every test outside it runs on no hosted leg. By the \
                 counts the ten-shard floors of 2026-11-20 were set from, the first six of \
                 those shards and the ninth, whose filters each contained `engine::`, were 50 + \
                 27 + 99 + 76 + 235 + 195 + 120 = 802 passing tests, and the rest of \
                 `engine::topology::`, which none of them named, was 484 tests by the counts \
                 the floors of 2026-11-19 were set from. 802 + 484 = 1,286 is above every one \
                 of the fourteen floors, the last leg's 985 among them: fourteen green legs \
                 over a suite that lost every test whose name does not contain `engine::`",
        job: Some("test-windows-hosted"),
        anchor: "          $env:RUSTC_BOOTSTRAP = '1'; foreach ($h in $harnesses) { $suite = Start-Process -FilePath $h -ArgumentList '${{ matrix.tests }} -Z unstable-options --report-time' -NoNewWindow -PassThru -RedirectStandardOutput \"$out.part\" -RedirectStandardError \"$err.part\"; $null = $suite.Handle; $suite.WaitForExit(); $code = $suite.ExitCode; try { Get-Content \"$out.part\" | Add-Content $out } catch { \"copying the output of $h failed: $_\"; if ($code -eq 0) { throw } }; try { Get-Content \"$err.part\" | Add-Content $err } catch { \"copying the errors of $h failed: $_\"; if ($code -eq 0) { throw } }; if ($code -ne 0) { break } }\n",
        replacement: "          $env:RUSTC_BOOTSTRAP = '1'; foreach ($h in $harnesses) { $suite = Start-Process -FilePath $h -ArgumentList 'engine:: -Z unstable-options --report-time' -NoNewWindow -PassThru -RedirectStandardOutput \"$out.part\" -RedirectStandardError \"$err.part\"; $null = $suite.Handle; $suite.WaitForExit(); $code = $suite.ExitCode; try { Get-Content \"$out.part\" | Add-Content $out } catch { \"copying the output of $h failed: $_\"; if ($code -eq 0) { throw } }; try { Get-Content \"$err.part\" | Add-Content $err } catch { \"copying the errors of $h failed: $_\"; if ($code -eq 0) { throw } }; if ($code -ne 0) { break } }\n",
        refused_as: "test-windows-hosted-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-MACOS-SHARD-NARROWED",
        escape: "the last macOS shard, the one with no filter, gains a `--skip effects::`. Every \
                 test of that shard whose name contains `effects::` then runs on no macOS leg, \
                 `topology::effects::registry::`'s among them. These legs hold no count to a \
                 floor, so all five are green over a macOS suite that lost those tests, beside \
                 a Linux leg that still runs them",
        job: Some("test-macos"),
        anchor: "          - leg: \"macos-latest, 5 of 5\"\n\
                 \x20           tests: \"",
        replacement: "          - leg: \"macos-latest, 5 of 5\"\n\
                      \x20           tests: \"--skip effects:: ",
        refused_as: "test-job-matrix",
    },
    WorkflowEscape {
        name: "MUT-TEST-MACOS-LAST-SHARD-SKIPS-MORE",
        escape: "the same skip put last in the last macOS leg's arguments, after the \
                 twenty-eight the table derives. Every test of that leg whose name contains \
                 `effects::` runs on no macOS leg, as under `MUT-TEST-MACOS-SHARD-NARROWED`, and \
                 no floor is there to say so; here the string opens as the table's does, so a \
                 reading that stopped at the table's last word would pass it",
        job: Some("test-macos"),
        anchor: "--skip engine::topology::attempt::\"\n    steps:\n",
        replacement: "--skip engine::topology::attempt:: --skip effects::\"\n    steps:\n",
        refused_as: "test-job-matrix",
    },
    WorkflowEscape {
        name: "MUT-TEST-MACOS-FILTER-DROPPED-FROM-ITS-LEG",
        escape: "the second macOS leg's first filter, \
                 `engine::topology::coordinator::tests::interleaving::`, deleted from that leg's \
                 own `tests:` line and from nowhere else. Every later macOS leg still skips it \
                 and the first never named it, so the coordinator's interleaving tests run on no \
                 macOS leg. These legs hold no count to a floor, so all five are green over a \
                 macOS suite that lost those tests, beside the Linux and Windows legs that \
                 still run them",
        job: Some("test-macos"),
        anchor: "            tests: \"engine::topology::coordinator::tests::interleaving:: engine::topology::coordinator::tests::a_s ",
        replacement: "            tests: \"engine::topology::coordinator::tests::a_s ",
        refused_as: "test-job-matrix",
    },
    WorkflowEscape {
        name: "MUT-TEST-MACOS-SKIP-DROPPED",
        escape: "the last macOS leg's first skip, `--skip engine::topology::coordinator::tests::a_p`, \
                 deleted. The first macOS leg runs the coordinator's tests whose names open \
                 `a_p`, and the last no longer excludes them, so they run twice on macOS, in \
                 the first leg and again in the last. These legs hold no count to a floor, and \
                 a repeat would only raise one: the last leg spends those tests' time a second \
                 time and every leg is green",
        job: Some("test-macos"),
        anchor: "          - leg: \"macos-latest, 5 of 5\"\n            tests: \"--skip engine::topology::coordinator::tests::a_p --skip ",
        replacement: "          - leg: \"macos-latest, 5 of 5\"\n            tests: \"--skip ",
        refused_as: "test-job-matrix",
    },
    WorkflowEscape {
        name: "MUT-TEST-MACOS-FILTER-MOVED-BETWEEN-LEGS",
        escape: "the third macOS leg's last filter, `events::log::`, moved to the front of the \
                 fourth leg's filters: one word moved and none lost. The fourth leg still skips \
                 it, as every macOS leg after the third does, and libtest runs no test a \
                 `--skip` names whatever filter selects it, so the event log's own tests run on \
                 no macOS leg. These legs hold no count to a floor, so all five are green",
        job: Some("test-macos"),
        anchor: " events::log:: --skip engine::topology::coordinator::tests::a_p --skip workspace_manager::tests::t --skip engine::tests::a_k --skip engine::tests::a_r --skip engine::tests::a_s --skip runner::host:: --skip workspace::tests:: --skip engine::topology::coordinator::tests::interleaving:: --skip engine::topology::coordinator::tests::a_s --skip workspace_manager::tests::e --skip workspace_manager::tests::n --skip topology::census:: --skip effects::tests::\"\n          - leg: \"macos-latest, 4 of 5\"\n            tests: \"",
        replacement: " --skip engine::topology::coordinator::tests::a_p --skip workspace_manager::tests::t --skip engine::tests::a_k --skip engine::tests::a_r --skip engine::tests::a_s --skip runner::host:: --skip workspace::tests:: --skip engine::topology::coordinator::tests::interleaving:: --skip engine::topology::coordinator::tests::a_s --skip workspace_manager::tests::e --skip workspace_manager::tests::n --skip topology::census:: --skip effects::tests::\"\n          - leg: \"macos-latest, 4 of 5\"\n            tests: \"events::log:: ",
        refused_as: "test-job-matrix",
    },
    WorkflowEscape {
        name: "MUT-TEST-UBUNTU-LEG-FILTERED",
        escape: "the last Linux leg, the one with no filter, handed one: its \
                 `tests: \"--skip engine::\"` becomes `tests: \"engine::\"`, the first Linux \
                 leg's. Both Linux legs then run the tests whose names contain `engine::` and \
                 neither runs any other; they hold no count to a floor, and both are green. The \
                 macOS shards and the Windows suite still run the rest on their platforms; the \
                 tests that run on Linux alone, the one that executes the aggregate's script \
                 among them, then run nowhere",
        job: Some("test-linux"),
        anchor: "          - leg: \"ubuntu-latest, 2 of 2\"\n            tests: \"--skip engine::\"\n",
        replacement: "          - leg: \"ubuntu-latest, 2 of 2\"\n            tests: \"engine::\"\n",
        refused_as: "test-job-matrix",
    },
    WorkflowEscape {
        name: "MUT-TEST-SHARD-SELECTION-DROPPED",
        escape: "`test-macos`'s cargo line loses the leg's selection and is the whole suite's \
                 again, the table untouched: each of its five legs runs the whole macOS suite, \
                 a leg that took 27 to 37 minutes while it was one, under the 30 minutes set \
                 for a shard. That is five macOS legs cancelled at their limit on a slow day, \
                 and five green legs and no shard on a fast one",
        job: Some("test-macos"),
        anchor: "      - run: cargo test --all-targets --all-features -- ${{ matrix.tests }}\n",
        replacement: "      - run: cargo test --all-targets --all-features\n",
        refused_as: "test-job-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-FILTER-HARDCODED",
        escape: "`test-linux`'s suite step names one filter, `engine::`, where it read the \
                 leg's `matrix.tests`, the table untouched. Both Linux legs then run the one \
                 subset, and neither holds a count to a floor: both are green and every test \
                 whose name does not contain `engine::` runs on no leg of this job. \
                 The line is a quoted scalar because a plain one cannot end in a colon: \
                 unquoted, it is a document the parser refuses before this contract reads it",
        job: Some("test-linux"),
        anchor: "      - run: cargo test --all-targets --all-features -- ${{ matrix.tests }}\n",
        replacement: "      - run: \"cargo test --all-targets --all-features -- engine::\"\n",
        refused_as: "test-job-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-LEGS-NAMED-BY-RUNNER",
        escape: "`test-macos` named by its runner again, `test (macos-latest)`, as the macOS \
                 leg was while each runner had one leg. The five macOS shards then publish that \
                 one name five times over, and a red one does not say which shard's tests \
                 failed",
        job: Some("test-macos"),
        anchor: "    name: test (${{ matrix.leg }})\n",
        replacement: "    name: test (macos-latest)\n",
        refused_as: "test-job-name",
    },
    WorkflowEscape {
        name: "MUT-TEST-TIMEOUT-RAISED-FOR-MACOS",
        escape: "the 45 minutes the macOS leg had while it ran the suite whole, put back on \
                 `test-macos` over the shards. Each macOS leg, a shard of that suite, may then \
                 run half as long again as the limit set for it, holding the required check \
                 and the queue entry behind it. Under one job this took an expression, which \
                 this contract cannot read as a number; with a job per platform it is one plain \
                 integer, which only the equality with the pinned 30 refuses",
        job: Some("test-macos"),
        anchor: "    timeout-minutes: 30\n",
        replacement: "    timeout-minutes: 45\n",
        refused_as: "test-job-timeout",
    },
    WorkflowEscape {
        name: "MUT-TEST-TIMEOUT-RAISED",
        escape: "the limit raised to 45 minutes on `test-linux` as well, the number the macOS \
                 leg had while it ran the suite whole, as a plain integer. It reads as a \
                 number, as `MUT-TEST-TIMEOUT-RAISED-FOR-MACOS`'s now does: only the equality \
                 with the pinned 30 refuses it. A Linux leg that never ends then holds the \
                 required check, and the merge-queue entry behind it, half as long again",
        job: Some("test-linux"),
        anchor: "    timeout-minutes: 30\n",
        replacement: "    timeout-minutes: 45\n",
        refused_as: "test-job-timeout",
    },
    WorkflowEscape {
        name: "MUT-TEST-MACOS-ON-LINUX",
        escape: "`test-macos`'s `runs-on: macos-latest` becomes `ubuntu-latest`. Every leg keeps \
                 its macOS name and its macOS shard's filters, and all five run on Linux: the \
                 macOS shards run elsewhere, every leg is green, and no test of this job runs on \
                 a Mac",
        job: Some("test-macos"),
        anchor: "    runs-on: macos-latest\n",
        replacement: "    runs-on: ubuntu-latest\n",
        refused_as: "test-job-matrix",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-SUITE-PIPED-AGAIN",
        escape: "line 7 of the shards' suite step runs each harness in a pipeline again, the \
                 harness over the leg's `matrix.tests` piped into `Tee-Object`: the pipeline \
                 the legs ran Cargo in, in runs 43719719561 and 44277078005, in each of which a \
                 shard reported its tests and did not end before its job's limit. By the \
                 inference the step is written against, a test process that outlives its \
                 harness holds that pipe, so a leg whose tests all passed is cancelled at its \
                 40 minutes with no failing test to show for it, on whichever of the fourteen legs \
                 runs the test that leaves the process",
        job: Some("test-windows-hosted"),
        anchor: "          $env:RUSTC_BOOTSTRAP = '1'; foreach ($h in $harnesses) { $suite = Start-Process -FilePath $h -ArgumentList '${{ matrix.tests }} -Z unstable-options --report-time' -NoNewWindow -PassThru -RedirectStandardOutput \"$out.part\" -RedirectStandardError \"$err.part\"; $null = $suite.Handle; $suite.WaitForExit(); $code = $suite.ExitCode; try { Get-Content \"$out.part\" | Add-Content $out } catch { \"copying the output of $h failed: $_\"; if ($code -eq 0) { throw } }; try { Get-Content \"$err.part\" | Add-Content $err } catch { \"copying the errors of $h failed: $_\"; if ($code -eq 0) { throw } }; if ($code -ne 0) { break } }\n",
        replacement: "          $env:RUSTC_BOOTSTRAP = '1'; foreach ($h in $harnesses) { & $h ${{ matrix.tests }} -Z unstable-options --report-time | Tee-Object -FilePath $out -Append; $code = $LASTEXITCODE; if ($code -ne 0) { break } }\n",
        refused_as: "test-windows-hosted-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-SURVIVOR-ALLOWED",
        escape: "line 11 of the shards' suite step, the throw for a test process left running, \
                 is gone, on all fourteen legs at once. Each leg still lists the `upstroke*` \
                 processes left and asks Windows to stop them, then counts and clears its \
                 floor: the leg whose tests leave a process is green beside the other thirteen, \
                 the process is forgotten whether or not it stopped, and which test leaves it, \
                 the question both runs that did not end left open, is asked by nothing",
        job: Some("test-windows-hosted"),
        anchor: "          if ($left.Count -gt 0) { throw \"$($left.Count) test process(es) outlived the suite, listed above: a test leaves a process running, which would hold this step open\" }\n",
        replacement: "",
        refused_as: "test-windows-hosted-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-BOOTSTRAP-BEFORE-THE-BUILD",
        escape: "`RUSTC_BOOTSTRAP` set on a line of its own above the build, where the loop's \
                 line sets it again. The build then runs in the bootstrap mode the variable \
                 exists for: Cargo and every compiler it starts accept unstable features, and a \
                 build script that asks whether the compiler does is answered yes, so the \
                 harnesses all fourteen legs test can be built otherwise than the ones the other \
                 platforms' legs build and a user builds, from a tree nobody changed",
        job: Some("test-windows-hosted"),
        anchor: "          $out = Join-Path $env:RUNNER_TEMP 'suite-stdout.log'; $err = Join-Path $env:RUNNER_TEMP 'suite-stderr.log'; $built = Join-Path $env:RUNNER_TEMP 'suite-build.json'\n",
        replacement: "          $env:RUSTC_BOOTSTRAP = '1'\n\
                      \x20         $out = Join-Path $env:RUNNER_TEMP 'suite-stdout.log'; $err = Join-Path $env:RUNNER_TEMP 'suite-stderr.log'; $built = Join-Path $env:RUNNER_TEMP 'suite-build.json'\n",
        refused_as: "test-windows-hosted-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-BOOTSTRAP-ON-THE-BUILD-LINE",
        escape: "`RUSTC_BOOTSTRAP` set on the build's own line, ahead of the `Start-Process` that \
                 builds, where the loop's line sets it again: the build of \
                 `MUT-TEST-WINDOWS-HOSTED-BOOTSTRAP-BEFORE-THE-BUILD`, in bootstrap mode, \
                 written where a reading that asked only whether a line above the build sets \
                 the variable would pass it",
        job: Some("test-windows-hosted"),
        anchor: "          $build = Start-Process -FilePath cargo -ArgumentList 'test --all-targets --all-features --no-run --message-format=json-render-diagnostics' -NoNewWindow -PassThru -RedirectStandardOutput $built -RedirectStandardError $err; $null = $build.Handle; $build.WaitForExit(); $code = $build.ExitCode\n",
        replacement: "          $env:RUSTC_BOOTSTRAP = '1'; $build = Start-Process -FilePath cargo -ArgumentList 'test --all-targets --all-features --no-run --message-format=json-render-diagnostics' -NoNewWindow -PassThru -RedirectStandardOutput $built -RedirectStandardError $err; $null = $build.Handle; $build.WaitForExit(); $code = $build.ExitCode\n",
        refused_as: "test-windows-hosted-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-CARGO-AFTER-BOOTSTRAP",
        escape: "a `cargo` command after the line that sets `RUSTC_BOOTSTRAP`: here the \
                 doctests, which `--all-targets` leaves out, run on a line of their own after \
                 the loop. The step's process still holds the variable, so that Cargo, every \
                 compiler it starts and every build script it runs work in bootstrap mode, and \
                 the doctests run on a build no other leg makes. The later steps run in \
                 processes of their own and do not inherit the variable; this line is in the \
                 step that sets it",
        job: Some("test-windows-hosted"),
        anchor: "          $env:RUSTC_BOOTSTRAP = '1'; foreach ($h in $harnesses) { $suite = Start-Process -FilePath $h -ArgumentList '${{ matrix.tests }} -Z unstable-options --report-time' -NoNewWindow -PassThru -RedirectStandardOutput \"$out.part\" -RedirectStandardError \"$err.part\"; $null = $suite.Handle; $suite.WaitForExit(); $code = $suite.ExitCode; try { Get-Content \"$out.part\" | Add-Content $out } catch { \"copying the output of $h failed: $_\"; if ($code -eq 0) { throw } }; try { Get-Content \"$err.part\" | Add-Content $err } catch { \"copying the errors of $h failed: $_\"; if ($code -eq 0) { throw } }; if ($code -ne 0) { break } }\n",
        replacement: "          $env:RUSTC_BOOTSTRAP = '1'; foreach ($h in $harnesses) { $suite = Start-Process -FilePath $h -ArgumentList '${{ matrix.tests }} -Z unstable-options --report-time' -NoNewWindow -PassThru -RedirectStandardOutput \"$out.part\" -RedirectStandardError \"$err.part\"; $null = $suite.Handle; $suite.WaitForExit(); $code = $suite.ExitCode; try { Get-Content \"$out.part\" | Add-Content $out } catch { \"copying the output of $h failed: $_\"; if ($code -eq 0) { throw } }; try { Get-Content \"$err.part\" | Add-Content $err } catch { \"copying the errors of $h failed: $_\"; if ($code -eq 0) { throw } }; if ($code -ne 0) { break } }\n\
                      \x20         cargo test --doc --all-features -- ${{ matrix.tests }}\n",
        refused_as: "test-windows-hosted-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-HARNESS-TIMES-DROPPED",
        escape: "the harnesses started without `-Z unstable-options --report-time`, the leg's \
                 `matrix.tests` still theirs: every leg runs its shard, clears its floor and is \
                 green, and no line of any leg's log carries the time a test took on the hosted \
                 runner. The times the step exists to record are lost on all fourteen legs at once, \
                 and nothing fails to say so",
        job: Some("test-windows-hosted"),
        anchor: "          $env:RUSTC_BOOTSTRAP = '1'; foreach ($h in $harnesses) { $suite = Start-Process -FilePath $h -ArgumentList '${{ matrix.tests }} -Z unstable-options --report-time' -NoNewWindow -PassThru -RedirectStandardOutput \"$out.part\" -RedirectStandardError \"$err.part\"; $null = $suite.Handle; $suite.WaitForExit(); $code = $suite.ExitCode; try { Get-Content \"$out.part\" | Add-Content $out } catch { \"copying the output of $h failed: $_\"; if ($code -eq 0) { throw } }; try { Get-Content \"$err.part\" | Add-Content $err } catch { \"copying the errors of $h failed: $_\"; if ($code -eq 0) { throw } }; if ($code -ne 0) { break } }\n",
        replacement: "          $env:RUSTC_BOOTSTRAP = '1'; foreach ($h in $harnesses) { $suite = Start-Process -FilePath $h -ArgumentList '${{ matrix.tests }}' -NoNewWindow -PassThru -RedirectStandardOutput \"$out.part\" -RedirectStandardError \"$err.part\"; $null = $suite.Handle; $suite.WaitForExit(); $code = $suite.ExitCode; try { Get-Content \"$out.part\" | Add-Content $out } catch { \"copying the output of $h failed: $_\"; if ($code -eq 0) { throw } }; try { Get-Content \"$err.part\" | Add-Content $err } catch { \"copying the errors of $h failed: $_\"; if ($code -eq 0) { throw } }; if ($code -ne 0) { break } }\n",
        refused_as: "test-windows-hosted-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-LOOP-PASSES-A-FAILED-HARNESS",
        escape: "the harness loop no longer stops at a harness that fails: each harness after it \
                 overwrites `$code`, so a failing harness followed by one that passes leaves \
                 `exit $code` reading zero, and the leg is judged by its count alone, which the \
                 harnesses that passed can clear: a failing test, on a green leg. Cargo's own \
                 `test` stops at the first harness that fails, and so does the loop",
        job: Some("test-windows-hosted"),
        anchor: "          $env:RUSTC_BOOTSTRAP = '1'; foreach ($h in $harnesses) { $suite = Start-Process -FilePath $h -ArgumentList '${{ matrix.tests }} -Z unstable-options --report-time' -NoNewWindow -PassThru -RedirectStandardOutput \"$out.part\" -RedirectStandardError \"$err.part\"; $null = $suite.Handle; $suite.WaitForExit(); $code = $suite.ExitCode; try { Get-Content \"$out.part\" | Add-Content $out } catch { \"copying the output of $h failed: $_\"; if ($code -eq 0) { throw } }; try { Get-Content \"$err.part\" | Add-Content $err } catch { \"copying the errors of $h failed: $_\"; if ($code -eq 0) { throw } }; if ($code -ne 0) { break } }\n",
        replacement: "          $env:RUSTC_BOOTSTRAP = '1'; foreach ($h in $harnesses) { $suite = Start-Process -FilePath $h -ArgumentList '${{ matrix.tests }} -Z unstable-options --report-time' -NoNewWindow -PassThru -RedirectStandardOutput \"$out.part\" -RedirectStandardError \"$err.part\"; $null = $suite.Handle; $suite.WaitForExit(); $code = $suite.ExitCode; try { Get-Content \"$out.part\" | Add-Content $out } catch { \"copying the output of $h failed: $_\"; if ($code -eq 0) { throw } }; try { Get-Content \"$err.part\" | Add-Content $err } catch { \"copying the errors of $h failed: $_\"; if ($code -eq 0) { throw } } }\n",
        refused_as: "test-windows-hosted-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-LOOP-DROPS-THE-FAILED-CODE",
        escape: "the harness loop stops at a harness that fails but does not keep its code: \
                 `$code` still holds the build's zero, `exit $code` does not fire, and the leg \
                 is judged by its count alone, which the harnesses that passed before the \
                 failing one can clear: a failing test, on a green leg",
        job: Some("test-windows-hosted"),
        anchor: "          $env:RUSTC_BOOTSTRAP = '1'; foreach ($h in $harnesses) { $suite = Start-Process -FilePath $h -ArgumentList '${{ matrix.tests }} -Z unstable-options --report-time' -NoNewWindow -PassThru -RedirectStandardOutput \"$out.part\" -RedirectStandardError \"$err.part\"; $null = $suite.Handle; $suite.WaitForExit(); $code = $suite.ExitCode; try { Get-Content \"$out.part\" | Add-Content $out } catch { \"copying the output of $h failed: $_\"; if ($code -eq 0) { throw } }; try { Get-Content \"$err.part\" | Add-Content $err } catch { \"copying the errors of $h failed: $_\"; if ($code -eq 0) { throw } }; if ($code -ne 0) { break } }\n",
        replacement: "          $env:RUSTC_BOOTSTRAP = '1'; foreach ($h in $harnesses) { $suite = Start-Process -FilePath $h -ArgumentList '${{ matrix.tests }} -Z unstable-options --report-time' -NoNewWindow -PassThru -RedirectStandardOutput \"$out.part\" -RedirectStandardError \"$err.part\"; $null = $suite.Handle; $suite.WaitForExit(); try { Get-Content \"$out.part\" | Add-Content $out } catch { \"copying the output of $h failed: $_\"; if ($code -eq 0) { throw } }; try { Get-Content \"$err.part\" | Add-Content $err } catch { \"copying the errors of $h failed: $_\"; if ($code -eq 0) { throw } }; if ($suite.ExitCode -ne 0) { break } }\n",
        refused_as: "test-windows-hosted-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-CODE-KEPT-AFTER-THE-COPIES",
        escape: "the harness loop keeps a harness's code only after copying its two outputs, the \
                 shape the step had when the legs first reported their tests' times: under \
                 `$ErrorActionPreference = 'stop'` a harness that exits 101 and whose standard \
                 output cannot be read ends the step on the copy's error, which the copy's \
                 `catch` throws again because `$code` still holds the build's zero or the last \
                 harness's. Not a false green, since the step still fails, but the failing \
                 harness's code is never kept and its standard error is never printed, so the \
                 leg's log says a file could not be read and not which test failed",
        job: Some("test-windows-hosted"),
        anchor: "          $env:RUSTC_BOOTSTRAP = '1'; foreach ($h in $harnesses) { $suite = Start-Process -FilePath $h -ArgumentList '${{ matrix.tests }} -Z unstable-options --report-time' -NoNewWindow -PassThru -RedirectStandardOutput \"$out.part\" -RedirectStandardError \"$err.part\"; $null = $suite.Handle; $suite.WaitForExit(); $code = $suite.ExitCode; try { Get-Content \"$out.part\" | Add-Content $out } catch { \"copying the output of $h failed: $_\"; if ($code -eq 0) { throw } }; try { Get-Content \"$err.part\" | Add-Content $err } catch { \"copying the errors of $h failed: $_\"; if ($code -eq 0) { throw } }; if ($code -ne 0) { break } }\n",
        replacement: "          $env:RUSTC_BOOTSTRAP = '1'; foreach ($h in $harnesses) { $suite = Start-Process -FilePath $h -ArgumentList '${{ matrix.tests }} -Z unstable-options --report-time' -NoNewWindow -PassThru -RedirectStandardOutput \"$out.part\" -RedirectStandardError \"$err.part\"; $null = $suite.Handle; $suite.WaitForExit(); try { Get-Content \"$out.part\" | Add-Content $out } catch { \"copying the output of $h failed: $_\"; if ($code -eq 0) { throw } }; try { Get-Content \"$err.part\" | Add-Content $err } catch { \"copying the errors of $h failed: $_\"; if ($code -eq 0) { throw } }; $code = $suite.ExitCode; if ($code -ne 0) { break } }\n",
        refused_as: "test-windows-hosted-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-OUTPUT-COPY-UNGUARDED",
        escape: "the copy of a harness's standard output loses its `try` and `catch`, the bare \
                 copy left where they stood: a `.part` file that cannot be read ends the step on \
                 PowerShell's error, with the harness's code kept and never used and its \
                 standard error never copied. Not a false green either, since a harness that \
                 passed throws on a failed copy as it does with the `catch`, but a harness that \
                 failed is reported as a file that could not be read, without its code or the \
                 errors that say which test failed",
        job: Some("test-windows-hosted"),
        anchor: "          $env:RUSTC_BOOTSTRAP = '1'; foreach ($h in $harnesses) { $suite = Start-Process -FilePath $h -ArgumentList '${{ matrix.tests }} -Z unstable-options --report-time' -NoNewWindow -PassThru -RedirectStandardOutput \"$out.part\" -RedirectStandardError \"$err.part\"; $null = $suite.Handle; $suite.WaitForExit(); $code = $suite.ExitCode; try { Get-Content \"$out.part\" | Add-Content $out } catch { \"copying the output of $h failed: $_\"; if ($code -eq 0) { throw } }; try { Get-Content \"$err.part\" | Add-Content $err } catch { \"copying the errors of $h failed: $_\"; if ($code -eq 0) { throw } }; if ($code -ne 0) { break } }\n",
        replacement: "          $env:RUSTC_BOOTSTRAP = '1'; foreach ($h in $harnesses) { $suite = Start-Process -FilePath $h -ArgumentList '${{ matrix.tests }} -Z unstable-options --report-time' -NoNewWindow -PassThru -RedirectStandardOutput \"$out.part\" -RedirectStandardError \"$err.part\"; $null = $suite.Handle; $suite.WaitForExit(); $code = $suite.ExitCode; Get-Content \"$out.part\" | Add-Content $out; try { Get-Content \"$err.part\" | Add-Content $err } catch { \"copying the errors of $h failed: $_\"; if ($code -eq 0) { throw } }; if ($code -ne 0) { break } }\n",
        refused_as: "test-windows-hosted-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-BUILD-RUNS-THE-SUITE",
        escape: "the build started without `--no-run`: Cargo runs every harness it builds, over \
                 the whole suite, with no selection and no times, and only then does the loop \
                 run the leg's shard again. Each of the fourteen legs runs the whole suite once and \
                 its own shard a second time, and on 2026-11-15 the whole suite alone took \
                 3,311 to 4,292 s in the harness on this runner, more than a leg's 40 minutes",
        job: Some("test-windows-hosted"),
        anchor: "          $build = Start-Process -FilePath cargo -ArgumentList 'test --all-targets --all-features --no-run --message-format=json-render-diagnostics' -NoNewWindow -PassThru -RedirectStandardOutput $built -RedirectStandardError $err; $null = $build.Handle; $build.WaitForExit(); $code = $build.ExitCode\n",
        replacement: "          $build = Start-Process -FilePath cargo -ArgumentList 'test --all-targets --all-features --message-format=json-render-diagnostics' -NoNewWindow -PassThru -RedirectStandardOutput $built -RedirectStandardError $err; $null = $build.Handle; $build.WaitForExit(); $code = $build.ExitCode\n",
        refused_as: "test-windows-hosted-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-BUILD-ERRORS-READ-UNGUARDED",
        escape: "the build's errors read outside a `try`, the line the step had before it read \
                 them in one: under `$ErrorActionPreference = 'stop'` a build that failed and \
                 whose errors file cannot be read ends the step with PowerShell's error, not \
                 Cargo's code, and without the errors that say what failed. Not a false green, \
                 since the step still fails, but the leg's log says a file could not be read \
                 and not why the build failed",
        job: Some("test-windows-hosted"),
        anchor: "          if ($code -ne 0) { try { Get-Content $err } catch { \"reading the build's errors failed: $_\" }; exit $code }\n",
        replacement: "          if ($code -ne 0) { Get-Content $err; exit $code }\n",
        refused_as: "test-windows-hosted-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-EMPTY-HARNESS-LIST-ALLOWED",
        escape: "the refusal of an empty list of harnesses is gone. Not a false green while the \
                 lines after it hold: with no harness run no output is written, and reading it \
                 fails the step, or a count of zero falls below the floor. But a leg whose \
                 build reported no test harness -- its messages read wrongly, or no harness of \
                 this crate built -- is then stopped by a missing file instead of a refusal \
                 that names the cause, and the floor is the last line between a shard that ran \
                 nothing and a green leg",
        job: Some("test-windows-hosted"),
        anchor: "          $harnesses = @(Get-Content $built | ForEach-Object { $_ | ConvertFrom-Json } | Where-Object { $_.reason -ceq 'compiler-artifact' -and $_.profile.test -and $_.executable } | ForEach-Object { $_.executable }); if ($harnesses.Count -eq 0) { throw \"Cargo built no test harness, so this shard would run nothing\" }\n",
        replacement: "          $harnesses = @(Get-Content $built | ForEach-Object { $_ | ConvertFrom-Json } | Where-Object { $_.reason -ceq 'compiler-artifact' -and $_.profile.test -and $_.executable } | ForEach-Object { $_.executable })\n",
        refused_as: "test-windows-hosted-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-RENAMED-AWAY",
        escape: "the job renamed: the pinned name has no job. Not a false green -- the \
                 aggregate's derived `needs` refuses the renamed workflow too -- but a lost \
                 handle: every pin on the Windows suite hangs from this name, so the contract \
                 must notice the name going before it notices nothing else",
        job: None,
        anchor: "\n  test-windows-hosted:\n",
        replacement: "\n  test-windows-shards:\n",
        refused_as: "test-windows-hosted-missing",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-CONDITIONED",
        escape: "a job-level `if:` that excludes the merge queue: the routing by event that \
                 this workflow dropped on 2026-11-20, put back. A pull request and a push still \
                 run the fourteen legs; on a queue entry the job is skipped, the aggregate reads \
                 `skipped` and fails, and no entry lands. Not a false green: the oracle refuses \
                 it before it is pushed, where the queue would refuse it one entry at a time",
        job: Some("test-windows-hosted"),
        anchor: "    runs-on: windows-latest\n",
        replacement: "    runs-on: windows-latest\n    if: github.event_name != 'merge_group'\n",
        refused_as: "unexpected-job-field",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-SUITE-ON-ONE-LANE",
        escape: "the merge-queue condition on the step that runs the suite. On a pull request \
                 or a push every leg skips that step and reports success; the queue still runs \
                 the suite, so the merge is still guarded, and every pull request is green on \
                 Windows having run none of its tests. No step of this job may carry an `if:`.",
        job: Some("test-windows-hosted"),
        anchor: "      - name: Test, and witness that the suite ran\n",
        replacement: "      - name: Test, and witness that the suite ran\n\
                      \x20       if: github.event_name == 'merge_group'\n",
        refused_as: "unexpected-step-field",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-RUN-RETARGETED",
        escape: "the shards' identity step fetches `master` and checks it out detached before \
                 the suite runs. The runner, the fields, the shells, the input-free checkout \
                 and the suite's script all still match; all fourteen legs test `master`, on every \
                 event, and are green over a candidate they never ran",
        job: Some("test-windows-hosted"),
        anchor: "          git config --global user.name \"upstroke CI\"\n",
        replacement: "          git config --global user.name \"upstroke CI\"\n\
                      \x20         git fetch origin master\n\
                      \x20         git checkout --detach FETCH_HEAD\n",
        refused_as: "test-windows-hosted-run-script",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-CAPTURE-CASEFOLD-DROPPED",
        escape: "the shards' capture no longer declares that the runner's NTFS folds case, so \
                 it runs under an environment its suite step did not, with its script still on \
                 its pin",
        job: Some("test-windows-hosted"),
        anchor: "      - name: Capture the native host acceptance record\n\
                 \x20       env:\n\
                 \x20         UPSTROKE_TEST_TEMP_FOLDS_CASE: \"1\"\n",
        replacement: "      - name: Capture the native host acceptance record\n",
        refused_as: "test-windows-hosted-capture-step-env",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-CAPTURE-STEP-DELETED",
        escape: "the shards' capture step is gone, its name, env and run. Every leg witnesses its \
                 suite, reports success and leaves no Windows host acceptance record, and no \
                 other job of this workflow makes one.",
        job: Some("test-windows-hosted"),
        anchor: r##"      - name: Capture the native host acceptance record
        env:
          UPSTROKE_TEST_TEMP_FOLDS_CASE: "1"
        run: |
          cargo test --all-features --lib engine::tests::host_acceptance_record -- --ignored --exact --nocapture | Tee-Object -Variable capture
          if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
          if (-not ($capture -ccontains 'test engine::tests::host_acceptance_record ... ok') -or -not ($capture -cmatch '^test result: ok\. 1 passed; 0 failed; 0 ignored; ')) { throw "the capture did not run exactly the one host acceptance test to a pass" }
          $f = @($capture | Where-Object { $_ -cmatch '^UPSTROKE-HOST-ACCEPTANCE-RECORD ' } | ForEach-Object { ,($_ -split ' ') })
          $ok = $f.Count -ge 3 -and $f[0].Count -eq 7 -and $f[-1].Count -eq 7 -and $f[0][1] -ceq 'v1' -and $f[0][2] -ceq 'BEGIN' -and $f[-1][1] -ceq 'v1' -and $f[-1][2] -ceq 'END' -and ($f[0][3..6] -join ' ') -ceq ($f[-1][3..6] -join ' ') -and $f[0][4] -ceq "$($f.Count - 2)"
          for ($k = 1; $ok -and $k -lt $f.Count - 1; $k++) { $ok = $f[$k].Count -eq 5 -and $f[$k][1] -ceq 'v1' -and $f[$k][2] -ceq $f[0][3] -and $f[$k][3] -ceq "$k" -and $f[$k][4].Length -eq $(if ($k -lt $f.Count - 2) { 192 } else { 2 * ([long]$f[0][5] - ([long]$f[0][4] - 1) * 96) }) }
          if (-not $ok) { throw "the host acceptance record is missing or its framing is incomplete" }
"##,
        replacement: "",
        refused_as: "test-windows-hosted-capture-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-CAPTURE-EXIT-SWALLOWED",
        escape: "the shards' capture no longer ends on Cargo's exit. A failing test or a Cargo \
                 that never built the harness ends the step on the checks after it, which name a \
                 missing pass line and not Cargo's failure; and where Cargo failed and still \
                 printed the pass line and a frame, no line of the script fails the step, which \
                 is left to the exit GitHub documents appending from `$LASTEXITCODE`",
        job: Some("test-windows-hosted"),
        anchor: "          cargo test --all-features --lib engine::tests::host_acceptance_record -- --ignored --exact --nocapture | Tee-Object -Variable capture\n\
                 \x20         if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }\n",
        replacement: "          cargo test --all-features --lib engine::tests::host_acceptance_record -- --ignored --exact --nocapture | Tee-Object -Variable capture\n",
        refused_as: "test-windows-hosted-capture-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-CHECKOUT-TWICE",
        escape: "a second input-free checkout immediately above the shards' capture. By the \
                 action's documented `clean: true` default it cleans the workspace, the ignored \
                 `target` among it, and checks the tree out again, so the capture builds and runs \
                 a harness the witness never counted, while the first checkout still matches",
        job: Some("test-windows-hosted"),
        anchor: "      - name: Capture the native host acceptance record\n",
        replacement: "      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0\n\
                      \x20     - name: Capture the native host acceptance record\n",
        refused_as: "test-windows-hosted-checkout",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-CHECKOUT-NOT-FIRST",
        escape: "a pinned cache step above the shards' checkout, which is no longer step 0. The \
                 step runs before the candidate is in the workspace, against an empty one or one \
                 another action prepared, and the checkout after it still matches its pin",
        job: Some("test-windows-hosted"),
        anchor: "      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0\n",
        replacement: "      - uses: Swatinem/rust-cache@6323deb102c322ba6fcbdcafc7e3dddab59af2b6 # v2.9.2\n\
                      \x20     - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0\n",
        refused_as: "test-windows-hosted-checkout",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-CAPABILITY-STEP-DELETED",
        escape: "the shards' observation step is gone (omitted). Every leg witnesses its suite \
                 and captures the host record, reports success and leaves no Windows container \
                 capability record, and no other job of this workflow makes one.",
        job: Some("test-windows-hosted"),
        anchor: r##"      - name: Capture the container capability observation
        timeout-minutes: 4
        env:
          UPSTROKE_TEST_TEMP_FOLDS_CASE: "1"
        run: |
          cargo test --all-features --lib runner::container::tests::container_capability_record -- --ignored --exact --nocapture | Tee-Object -Variable capture
          if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
          if (-not ($capture -ccontains 'test runner::container::tests::container_capability_record ... ok') -or -not ($capture -cmatch '^test result: ok\. 1 passed; 0 failed; 0 ignored; ')) { throw "the observation did not run exactly the one container capability test to a pass" }
          $f = @($capture | Where-Object { $_ -cmatch '^UPSTROKE-CONTAINER-CAPABILITY-RECORD ' } | ForEach-Object { ,($_ -split ' ') })
          $ok = $f.Count -ge 3 -and $f[0].Count -eq 7 -and $f[-1].Count -eq 7 -and $f[0][1] -ceq 'v1' -and $f[0][2] -ceq 'BEGIN' -and $f[-1][1] -ceq 'v1' -and $f[-1][2] -ceq 'END' -and ($f[0][3..6] -join ' ') -ceq ($f[-1][3..6] -join ' ') -and $f[0][4] -ceq "$($f.Count - 2)"
          for ($k = 1; $ok -and $k -lt $f.Count - 1; $k++) { $ok = $f[$k].Count -eq 5 -and $f[$k][1] -ceq 'v1' -and $f[$k][2] -ceq $f[0][3] -and $f[$k][3] -ceq "$k" -and $f[$k][4].Length -eq $(if ($k -lt $f.Count - 2) { 192 } else { 2 * ([long]$f[0][5] - ([long]$f[0][4] - 1) * 96) }) }
          if (-not $ok) { throw "the container capability record is missing or its framing is incomplete" }
"##,
        replacement: "",
        refused_as: "test-windows-hosted-capability-command",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-CAPABILITY-TIMEOUT-DROPPED",
        escape: "the 4-minute bound on the shards' observation is gone: a stalled question is \
                 held by the leg's own 40 minutes, past the step's budget",
        job: Some("test-windows-hosted"),
        anchor: "        timeout-minutes: 4\n",
        replacement: "",
        refused_as: "test-windows-hosted-capability-step",
    },
    WorkflowEscape {
        name: "MUT-TEST-WINDOWS-HOSTED-CAPABILITY-CASEFOLD-DROPPED",
        escape: "the shards' observation no longer declares that the runner's NTFS folds case, \
                 so it runs under an environment its suite step did not",
        job: Some("test-windows-hosted"),
        anchor: "        timeout-minutes: 4\n\
                 \x20       env:\n\
                 \x20         UPSTROKE_TEST_TEMP_FOLDS_CASE: \"1\"\n",
        replacement: "        timeout-minutes: 4\n",
        refused_as: "test-windows-hosted-capability-step-env",
    },
    WorkflowEscape {
        name: "MUT-JOB-PERMISSIONS-ON-THE-HOSTED-SHARDS",
        escape: "a `permissions:` block on the shards' job, reading actions and contents. A \
                 block on a job replaces the workflow's for that job, and this job runs the \
                 candidate's build scripts and tests, so whatever the block names is theirs: \
                 here a token that reads other runs. The field set refuses the key as well, \
                 and the permission scan is what names the cause.",
        job: Some("test-windows-hosted"),
        anchor: "    timeout-minutes: 40\n",
        replacement: "    timeout-minutes: 40\n\
                      \x20   permissions:\n\
                      \x20     actions: read\n\
                      \x20     contents: read\n",
        refused_as: "job-permissions",
    },
    WorkflowEscape {
        name: "MUT-AGGREGATE-ACCEPTS-THE-HOSTED-SHARDS-SKIPPED",
        escape: "the aggregate accepts `skipped` from the hosted shards: the old rule's \
                 acceptance of a skipped Windows suite job, put back for the one job left. The \
                 loop still names all seven gates and every other one must still succeed, so a \
                 run whose shards are skipped is green with no Windows suite: under \
                 `MUT-TEST-WINDOWS-HOSTED-CONDITIONED`'s `if:`, every queue entry",
        job: Some("merge-gate"),
        anchor: "            if [[ \"$result\" != \"success\" ]]; then\n",
        replacement: "            if [[ \"$result\" != \"success\" && \"$gate=$result\" != \"TEST_WINDOWS_HOSTED=skipped\" ]]; then\n",
        refused_as: "aggregate-script",
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

pub(super) fn shard_selections(table: &[&[&str]]) -> Vec<String> {
    (0..=table.len())
        .map(|shard| {
            let skips = table
                .iter()
                .take(shard)
                .flat_map(|earlier| earlier.iter())
                .flat_map(|filter| ["--skip", *filter]);
            table
                .get(shard)
                .copied()
                .unwrap_or_default()
                .iter()
                .copied()
                .chain(skips)
                .collect::<Vec<&str>>()
                .join(" ")
        })
        .collect()
}

pub(super) fn libtest_runs(selection: &str, name: &str) -> bool {
    let mut filters: Vec<&str> = Vec::new();
    let mut skips: Vec<&str> = Vec::new();
    let mut words = selection.split_whitespace();
    while let Some(word) = words.next() {
        if word != "--skip" {
            filters.push(word);
            continue;
        }
        let Some(skipped) = words.next() else {
            return false;
        };
        skips.push(skipped);
    }
    (filters.is_empty() || filters.iter().any(|filter| name.contains(*filter)))
        && !skips.iter().any(|skip| name.contains(*skip))
}
