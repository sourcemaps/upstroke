//! Extended notes: `docs/internals/engine/topology/run/tests.md`

use super::*;
use crate::topology::events::{AttemptNumber, DerivedOutcome, GenerationId};
use crate::topology::registry::TaskKey;

#[test]
fn the_transcribed_loop_branches_are_the_packets_seven() {
    assert_eq!(
        LoopBranch::ALL
            .iter()
            .map(|branch| branch.label())
            .collect::<Vec<_>>(),
        vec![
            "ingest answers",
            "integration",
            "ready_retry",
            "ready dispatch",
            "defer backoff",
            "hard block",
            "run-end closure",
        ],
        "transcribed from `decisions.sequential_substrate.loop`, in its order"
    );
}

#[test]
fn every_branch_states_what_this_build_does_with_it() {
    let refused: Vec<&str> = LoopBranch::ALL
        .iter()
        .filter(|branch| branch.disposition() == Disposition::RefusedByCheckpoint)
        .map(|branch| branch.label())
        .collect();
    assert!(
        refused.is_empty(),
        "no branch of the loop is refused by this build: run-end closure, which PR7 through \
         PR9 refused at the checkpoint, is performed since PR10, so `checkpoint_refusals` \
         names no terminal this build does not implement. A branch here is a build refusing \
         something the packet did not let it refuse: {refused:?}"
    );

    let owed: Vec<&str> = LoopBranch::ALL
        .iter()
        .filter(|branch| branch.disposition() == Disposition::NotYetImplemented)
        .map(|branch| branch.label())
        .collect();
    assert!(
        owed.is_empty(),
        "the branches this build has not written. Every one of them is carried \
         in the type so that no instrument here has to notice its absence. \
         `defer backoff` left this list when `TopologyRun::step` grew its arm, \
         which is the shape every entry here is expected to leave by. It is \
         empty now: {owed:?}"
    );

    let elsewhere: Vec<&str> = LoopBranch::ALL
        .iter()
        .filter(|branch| matches!(branch.disposition(), Disposition::NotThisSlice { .. }))
        .map(|branch| branch.label())
        .collect();
    assert!(
        elsewhere.is_empty(),
        "every branch of the loop is this build's: `ingest answers` was PR9's by \
         `pr_sequence[10]` (`T-ANSWER`, `AwaitingInput -> Pending via validated answer`) and \
         PR9 performs it before every selection. A branch here left this build's scope \
         without saying which slice took it: {elsewhere:?}"
    );

    assert_eq!(
        LoopBranch::ReadyDispatch.disposition(),
        Disposition::Performed,
        "`loop` states this branch as four clauses and this build performs \
         three; the type says which three"
    );
}

#[test]
fn every_step_belongs_to_one_branch_or_to_none_for_a_reason() {
    let cases: Vec<(Step, Option<LoopBranch>)> = vec![
        (Step::Poisoned, None),
        (
            Step::Retry {
                key: TaskKey(0),
                generation: GenerationId(0),
                attempt: AttemptNumber(1),
            },
            Some(LoopBranch::ReadyRetry),
        ),
        (
            Step::Dispatch {
                key: TaskKey(0),
                generation: GenerationId(0),
                continuing: false,
            },
            Some(LoopBranch::ReadyDispatch),
        ),
        (
            Step::RepairDispatch {
                key: TaskKey(2),
                generation: GenerationId(0),
                continuing: true,
            },
            Some(LoopBranch::ReadyDispatch),
        ),
        (Step::Backoff, Some(LoopBranch::DeferBackoff)),
        (
            Step::HardBlock {
                questions: Vec::new(),
            },
            Some(LoopBranch::HardBlock),
        ),
        (
            Step::Closure(DerivedOutcome::NotEnding),
            Some(LoopBranch::Closure),
        ),
    ];
    for (step, expected) in cases {
        assert_eq!(
            LoopBranch::of(&step),
            expected,
            "`{step:?}` maps to the wrong branch"
        );
    }
}

#[test]
fn a_refusal_names_the_branch_and_says_whether_anything_happened() {
    let untouched = LoopBranch::HardBlock.unimplemented().to_string();
    assert!(
        untouched.contains("hard block"),
        "the refusal names the branch: {untouched}"
    );
    assert!(
        untouched.contains("no effect was performed")
            && untouched.contains("no event was appended"),
        "and says the run is untouched: {untouched}"
    );

    assert!(
        LoopBranch::ALL
            .iter()
            .all(|branch| !matches!(branch.disposition(), Disposition::PartlyImplemented { .. })),
        "a branch is partly built again — assert its `performed ... does not ...` \
         message here, because an operator reading `not implemented` would look \
         for a run that had not started"
    );

    for branch in LoopBranch::ALL {
        let refusal = branch.unimplemented().to_string();
        assert!(
            refusal.contains(branch.label()),
            "`{}`'s refusal does not name it: {refusal}",
            branch.label()
        );
    }
}

#[test]
fn every_driver_append_propagates_its_error() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/engine/topology/run.rs"),
    )
    .expect("the driver's own source");
    let code = crate::effects::production_code(&source);

    assert!(
        code.len() * 10 > source.len(),
        "the production region is {} of {} bytes — a census over a fraction of a \
         file reports zero for the part it never read",
        code.len(),
        source.len()
    );

    let needle = "self.emit(";
    let mut sites = 0;
    let mut unpropagated = Vec::new();
    for (at, _) in code.match_indices(needle) {
        sites += 1;
        let mut depth = 0_i32;
        let mut end = None;
        for (offset, ch) in code[at + needle.len() - 1..].char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(at + needle.len() - 1 + offset + 1);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(end) = end else {
            unpropagated.push(format!("unbalanced call at byte {at}"));
            continue;
        };
        if !code[end..].trim_start().starts_with('?') {
            let line = code[..at].matches('\n').count() + 1;
            unpropagated.push(format!("line {line} (of the blanked region)"));
        }
    }

    assert!(
        sites >= 4,
        "only {sites} append sites found, so a green result here would prove nothing"
    );
    assert!(
        unpropagated.is_empty(),
        "these driver appends do not propagate their error, so the append-error \
         protocol never runs for them: {unpropagated:?}"
    );
}

#[test]
fn the_loop_selects_through_one_function() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/engine/topology/run.rs"),
    )
    .expect("the driver's own source");
    let code = crate::effects::production_code(&source);

    assert!(
        code.len() * 10 > source.len(),
        "the production region is {} of {} bytes",
        code.len(),
        source.len()
    );

    let calls = |needle: &str| {
        code.match_indices(needle)
            .filter(|(at, _)| !code[..*at].trim_end().ends_with("fn"))
            .count()
    };

    assert_eq!(
        calls("select("),
        1,
        "the driver reaches its branch order through {} calls to `select`. Zero \
         means a second selector was written and this one bypassed — the branch \
         order the packet specifies is then not the order the run takes, and \
         `select`'s own tests still pass",
        calls("select(")
    );
    assert_eq!(
        calls("checkpoint("),
        1,
        "`checkpoint` refuses the terminals this build does not implement. One \
         selector guarded by one checkpoint is the pair; a selected step that \
         reached the loop unguarded is `INV-07`'s failure"
    );
}

const DRIVEN_TRANSITIONS: [(&str, usize, usize); 5] = [
    ("begin_dispatch", 1, 1),
    ("begin_retry", 1, 1),
    ("settle_judged", 2, 1),
    ("close_run", 2, 2),
    ("hard_block", 1, 1),
];

fn transition_calls(code: &str, transition: &str) -> Vec<(String, bool)> {
    let needle = format!("{transition}(");
    let bytes = code.as_bytes();
    code.match_indices(&needle)
        .filter_map(|(at, _)| {
            let before = at
                .checked_sub(1)
                .and_then(|index| bytes.get(index))
                .copied();
            if before.is_some_and(|byte| byte.is_ascii_alphanumeric() || byte == b'_') {
                return None;
            }
            let rest = code.get(at + needle.len()..)?;
            let mut depth = 0_usize;
            let mut end = rest.len();
            for (index, character) in rest.char_indices() {
                match character {
                    '(' | '[' | '{' => depth += 1,
                    ')' | ']' | '}' if depth == 0 => {
                        end = index;
                        break;
                    }
                    ')' | ']' | '}' => depth -= 1,
                    ',' if depth == 0 => {
                        end = index;
                        break;
                    }
                    _ => {}
                }
            }
            let operator = rest
                .get(..end)?
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            Some((operator, before == Some(b'.')))
        })
        .collect()
}

fn transition_bypasses(run: &str, coordinator: &str) -> Vec<String> {
    let mut bypasses = Vec::new();
    for (transition, in_run, in_coordinator) in DRIVEN_TRANSITIONS {
        let defined = run
            .matches(&format!(
                "pub(super) fn {transition}<O: Operator + ?Sized>("
            ))
            .count();
        if defined != 1 {
            bypasses.push(format!(
                "`{transition}` is defined {defined} time(s) generic over its operator, not once"
            ));
        }
        let calls = transition_calls(run, transition);
        if calls.len() != in_run {
            bypasses.push(format!(
                "run.rs calls `{transition}` {} time(s), not {in_run}: {calls:?}",
                calls.len()
            ));
        }
        for (operator, method) in &calls {
            if *method
                || !["&mut self.stepping(seams, hooks)", "operator"].contains(&operator.as_str())
            {
                bypasses.push(format!(
                    "run.rs calls `{transition}` with `{operator}`, which is neither the width-1 \
                     step's own `Stepping` nor the operator it was handed"
                ));
            }
        }
        let calls = transition_calls(coordinator, transition);
        if calls.len() != in_coordinator {
            bypasses.push(format!(
                "coordinator.rs calls `{transition}` {} time(s), not {in_coordinator}: {calls:?}",
                calls.len()
            ));
        }
        for (operator, method) in &calls {
            if *method || operator != "self" {
                bypasses.push(format!(
                    "coordinator.rs calls `{transition}` with `{operator}`, not with itself as the \
                     operator"
                ));
            }
        }
    }
    bypasses
}

fn driver_sources() -> (String, String) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let read = |file: &str| {
        crate::effects::production_code(
            &std::fs::read_to_string(root.join(file))
                .expect("a source file")
                .replace("\r\n", "\n"),
        )
    };
    (
        read("src/engine/topology/run.rs"),
        read("src/engine/topology/coordinator.rs"),
    )
}

#[test]
fn both_drivers_run_each_transition_through_the_one_generic_function() {
    let (run, coordinator) = driver_sources();
    for (file, code, anchors) in [
        (
            "run.rs",
            &run,
            &["pub fn step(", "fn emit_undischarged("][..],
        ),
        (
            "coordinator.rs",
            &coordinator,
            &[
                "fn idle(&mut self)",
                "fn finish(&mut self)",
                "fn registry_pause(",
            ][..],
        ),
    ] {
        for anchor in anchors {
            assert!(
                code.contains(anchor),
                "the census reads {file} without `{anchor}`, so its domain is not the drivers' \
                 production code"
            );
        }
    }
    let bypasses = transition_bypasses(&run, &coordinator);
    assert!(
        bypasses.is_empty(),
        "every call of every transition runs through the one generic function, by the width-1 \
         step's own `Stepping` or by the coordinator as itself:\n{}",
        bypasses.join("\n")
    );
}

#[test]
fn the_r_t_census_reports_one_call_that_bypasses_its_driver() {
    let (run, coordinator) = driver_sources();
    assert!(transition_bypasses(&run, &coordinator).is_empty());
    let retry_arm = "settle_judged(&mut self.stepping(seams, hooks), manager, &job, &judged)";
    assert_eq!(
        run.matches(retry_arm).count(),
        2,
        "the two width-1 settlements"
    );
    for foreign in [
        "settle_judged(&mut Stepping { run: self, seams, hooks: &mut super::seams::NoTopologyHooks::new() }, manager, &job, &judged)",
        "settle_judged(&mut self.stepping(seams, &mut super::seams::NoTopologyHooks::new()), manager, &job, &judged)",
    ] {
        let one = run.replacen(retry_arm, foreign, 1);
        let bypasses = transition_bypasses(&one, &coordinator);
        assert_eq!(
            bypasses.len(),
            1,
            "the census reports the one settlement run through a foreign operator: {bypasses:?}"
        );
    }
    let mut coordinator_bypass = coordinator.replacen(
        "super::run::settle_judged(self, manager, &job, &judged)",
        "super::run::settle_judged(&mut super::run::Stepping { run: self.run, seams: self.seams, hooks: self.hooks }, manager, &job, &judged)",
        1,
    );
    assert_ne!(
        coordinator_bypass, coordinator,
        "the coordinator's settlement is spelled as expected"
    );
    assert_eq!(transition_bypasses(&run, &coordinator_bypass).len(), 1);
    coordinator_bypass = coordinator.replacen(
        "super::run::close_run(self, seams, ",
        "self.run.close_run(",
        1,
    );
    assert_ne!(
        coordinator_bypass, coordinator,
        "the coordinator's closure is spelled as expected"
    );
    assert!(
        !transition_bypasses(&run, &coordinator_bypass).is_empty(),
        "a closure called as the run's own method is reported"
    );
}

#[test]
fn the_frozen_pool_table_is_read_through_one_seam() {
    const FILE: &str = "src/engine/assembly.rs";

    let source =
        std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(FILE))
            .expect("a source file");
    let code = crate::effects::production_code(&source);
    assert!(
        code.len() * 2 > source.len(),
        "the production region of {FILE} is {} of {} bytes, so a count over it says little about \
         the file",
        code.len(),
        source.len()
    );

    use crate::effects::census_domain::{Call, production_calls};

    let calls = production_calls(&code, "pool_for", Call::Free);
    assert_eq!(
        calls, 1,
        "{FILE} resolves an agent's pool from the frozen table in {calls} places. One is \
         `AttemptPlans::pool_for`, which is the seam every caller is supposed to ask; a second is \
         a rule with two implementations, and `wrong_internal_assumption` is how this project \
         pays for those"
    );

    assert_eq!(
        production_calls(
            "use crate::capacity::pool_for;\nfn second() { pool_for(agent, pools); }\n",
            "pool_for",
            Call::Free,
        ),
        1,
        "the needle this census reads {FILE} with does not see a bare `pool_for(` behind a \
         `use`, which is how a second implementation is ordinarily written"
    );
    assert_eq!(
        production_calls(
            "fn asks() { self.pool_for(agent); }\n",
            "pool_for",
            Call::Free
        ),
        0,
        "the needle counts the seam's own callers, so every caller asking correctly would be \
         reported as a second implementation"
    );
}

fn attempt_started_constructions(code: &str) -> Vec<usize> {
    code.match_indices("AttemptStarted4 {")
        .filter(|(at, _)| !code[..*at].trim_end().ends_with("struct"))
        .map(|(at, _)| at)
        .collect()
}

#[test]
fn both_attempt_started_arms_take_their_pool_from_an_authority() {
    const SITES: &[(&str, &str)] = &[
        (
            "src/engine/topology/attempt.rs",
            "the dispatch arm: `plan.pool`, resolved by the assembler that owns the pool table",
        ),
        (
            "src/engine/topology/settle.rs",
            "the retry arm: `request.pool`, which the driver fills from the record of the retained \
             attempt whose session the retry resumes",
        ),
    ];

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source_root = root.join("src");
    let mut all = Vec::new();
    let mut stack = vec![source_root.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("src is readable") {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                all.push(path);
            }
        }
    }
    let test_modules =
        crate::effects::census_domain::whole_file_test_modules(&source_root, &all, 13);
    let mut arms: Vec<(String, usize)> = Vec::new();
    for path in &all {
        if test_modules.contains(path) {
            continue;
        }
        let source = std::fs::read_to_string(path).expect("a source file");
        let found = attempt_started_constructions(&crate::effects::production_code(&source)).len();
        if found > 0 {
            let relative = path
                .strip_prefix(root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            arms.push((relative, found));
        }
    }
    arms.sort();
    assert_eq!(
        arms,
        SITES
            .iter()
            .map(|(file, _)| ((*file).to_owned(), 1))
            .collect::<Vec<_>>(),
        "production appends `attempt_started` from exactly these two arms, once each; an arm this \
         list does not name is an arm whose pool nothing checks"
    );

    let mut invented: Vec<String> = Vec::new();
    let mut checked = 0_usize;
    for (file, why) in SITES {
        let source = std::fs::read_to_string(root.join(file)).expect("a source file");
        let code = crate::effects::production_code(&source);
        for at in attempt_started_constructions(&code) {
            let rest = &code[at..];
            let body = &rest[..rest.find("})").unwrap_or(rest.len())];
            let pool = body
                .lines()
                .find_map(|line| line.trim().strip_prefix("pool:"))
                .unwrap_or_else(|| panic!("{file}'s `AttemptStarted4` has no `pool` field"));
            checked += 1;
            if pool.trim().starts_with("None") {
                invented.push(format!("{file} — {why}"));
            }
        }
    }

    assert_eq!(checked, SITES.len(), "a site stopped being found");
    assert!(
        invented.is_empty(),
        "these append `attempt_started` with a hard-coded `pool: None`, so the ledger and the \
         plan disagree about which pool the attempt drained: {invented:?}"
    );
    assert_eq!(
        attempt_started_constructions(
            "pub struct AttemptStarted4 {}\nfn a() { x(AttemptStarted4 { pool: None }) }\n\
             fn b() { y(AttemptStarted4 {\n pool: None }) }\n"
        )
        .len(),
        2,
        "every construction is counted, the second as well as the first, and the definition is not"
    );
}

#[test]
fn the_settled_notes_separate_the_successful_and_the_failed_settlement() {
    const NOTES: &str = include_str!("../../../../docs/internals/engine/topology/run.md");

    let settled = NOTES
        .split("\n## ")
        .find(|section| section.starts_with("`pub enum Progress` › `Settled {`"))
        .expect("the notes carry the `Settled {` heading the branch summary sits under");
    let settled = settled.split_whitespace().collect::<Vec<_>>().join(" ");

    for (proposition, pin) in [
        (
            "which settlement is appended depends on `accepted`",
            "depends on `accepted`",
        ),
        (
            "a rejected attempt settles with `attempt_finished`",
            "rejected attempt ends at `attempt_finished`",
        ),
        (
            "an accepted attempt appends no `attempt_finished`",
            "never appends `attempt_finished`",
        ),
        (
            "an accepted attempt settles at `candidate_prepared`",
            "`candidate_prepared`",
        ),
        (
            "and `task_candidate_created` follows it",
            "`task_candidate_created`",
        ),
    ] {
        assert!(
            settled.contains(pin),
            "the `Settled {{` summary must state that {proposition}; looked for {pin:?} in:\n{settled}"
        );
    }

    assert!(
        !settled.contains("the attempt through the Runner, and `attempt_finished`."),
        "the retired claim that the whole ready-dispatch branch ends in \
         `attempt_finished` must not come back:\n{settled}"
    );
}

#[test]
fn the_ready_branch_notes_do_not_owe_the_attempt_the_branch_runs() {
    const NOTES: &str = include_str!("../../../../docs/internals/engine/topology/run.md");
    const ARM: &str = "`pub const fn disposition(self) -> Disposition` › `Self::";

    let section = |heading: &str| -> String {
        NOTES
            .split("\n## ")
            .find(|section| section.starts_with(heading))
            .map(|section| section.split_whitespace().collect::<Vec<_>>().join(" "))
            .unwrap_or_else(|| panic!("the notes carry no {heading:?} heading"))
    };

    for (branch, variant, states, retired) in [
        (
            LoopBranch::ReadyDispatch,
            "ReadyDispatch",
            &[
                (
                    "the branch performs all four of its clauses",
                    "All four clauses",
                ),
                (
                    "the fourth of which is the attempt and its settlement",
                    "run one attempt through the Runner and settle",
                ),
            ][..],
            &[
                (
                    "only the first three clauses are performed",
                    "The first three are here",
                ),
                (
                    "the branch stops before the attempt, at `OpenNoAttempt`",
                    "leaves instead is `OpenNoAttempt`",
                ),
            ][..],
        ),
        (
            LoopBranch::ReadyRetry,
            "ReadyRetry",
            &[
                (
                    "the branch performs its clause whole",
                    "generation\", whole:",
                ),
                (
                    "the attempt and its settlement included",
                    "the attempt itself and its settlement",
                ),
                (
                    "reached through the ready-dispatch branch's own machinery",
                    "the same `attempt` and `settle`",
                ),
            ][..],
            &[(
                "running and settling the retry is still owed",
                "half still owed",
            )][..],
        ),
    ] {
        assert_eq!(
            branch.disposition(),
            Disposition::Performed,
            "`{}` is no longer `Performed`; these pins describe a branch that \
             runs its attempt and settles it, so they are the wrong assertions \
             for whatever it does now",
            branch.label()
        );

        let notes = section(&format!("{ARM}{variant} => Disposition::Performed,`"));
        for (proposition, pin) in states {
            assert!(
                notes.contains(pin),
                "the `{variant}` section must state that {proposition}; looked \
                 for {pin:?} in:\n{notes}"
            );
        }
        for (claim, pin) in retired {
            assert!(
                !notes.contains(pin),
                "the retired claim that {claim} must not come back — \
                 `TopologyRun::step` and `TopologyRun::retry_ready` both reach \
                 `attempt` and `settle`; found {pin:?} in:\n{notes}"
            );
        }
    }

    let partly = section("`pub enum Disposition` › `PartlyImplemented {`");
    assert!(
        partly.contains("No branch is `PartlyImplemented` today"),
        "the `PartlyImplemented` section must say the variant has no \
         inhabitants, because its example is a build that no longer \
         exists:\n{partly}"
    );
    assert!(
        !partly.contains("the ready-dispatch branch's first three clauses are"),
        "the retired claim that the ready-dispatch branch is presently \
         half-built must not come back:\n{partly}"
    );
}

#[test]
fn the_closure_notes_say_what_closure_does_under_concurrency_and_what_it_still_refuses() {
    const NOTES: &str = include_str!("../../../../docs/internals/engine/topology/run.md");
    const HEADING: &str = "`pub const fn disposition(self) -> Disposition` › `Self::Closure => Disposition::Performed,`";

    let closure = NOTES
        .split("\n## ")
        .find(|section| section.starts_with(HEADING))
        .map(|section| section.split_whitespace().collect::<Vec<_>>().join(" "))
        .unwrap_or_else(|| panic!("the notes carry no {HEADING:?} heading"));
    for (proposition, pin) in [
        (
            "closure under concurrency is phase 4's, in `close_run`",
            "The concurrent half of closure is PR11 phase 4's, done in the same `close_run`",
        ),
        (
            "a halt's vouched identities are settled interrupted",
            "settles interrupted with their residue",
        ),
        (
            "a budget stop drains before the closure runs",
            "a budget stop drains its pipelines before the closure runs",
        ),
        (
            "what is still refused is in-flight work nothing vouches for",
            "is in-flight work nothing vouches for",
        ),
    ] {
        assert!(
            closure.contains(pin),
            "the `Self::Closure` section must state that {proposition}; looked for {pin:?} \
             in:\n{closure}"
        );
    }
    assert!(
        !closure.contains("is refused by `closure::refuse_unclosable` naming PR11"),
        "the retired claim that closure under concurrency is refused must not come back — \
         `TopologyRun::close_run` settles, promotes and publishes it:\n{closure}"
    );
}

#[test]
fn the_verification_notes_say_a_registry_another_process_is_writing_never_reaches_the_git_arm() {
    use crate::engine::topology::attempt::JudgeError;
    use crate::engine::topology::integrate::Verified;
    use crate::topology::events::SequenceId;

    const NOTES: &str = include_str!("../../../../docs/internals/engine/topology/run.md");
    const HEADING: &str = "`impl Verification for IntegrationCx<'_, '_>` › `Err(JudgeError::Other(UpstrokeError::Git { message })) => Ok(Verified::Unavailable {`";

    let arm = NOTES
        .split("\n## ")
        .find(|section| section.starts_with(HEADING))
        .map(|section| section.split_whitespace().collect::<Vec<_>>().join(" "))
        .unwrap_or_else(|| panic!("the notes carry no {HEADING:?} heading"));
    for (proposition, pin) in [
        (
            "a registry another process is half-way through writing never reaches this arm",
            "A registry another process is half-way through writing never reaches this arm",
        ),
        (
            "every registry access is attempted again until its deadline",
            "attempted again until its deadline",
        ),
        (
            "a registration still in the way then refuses as a registry refusal",
            "refuses as `UpstrokeError::RegistryRefused`",
        ),
        (
            "the command then ends resumably with nothing appended",
            "the command ends resumably, with the transaction open and nothing appended",
        ),
        (
            "a coordinator in a linked checkout of the same repository was the case",
            "`PR11-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY`",
        ),
        (
            "a genuine checkout failure after Git's takeover no longer reaches it",
            "`PR329-A-GENUINE-CHECKOUT-FAILURE-AFTER-THE-TAKEOVER-REFUSES`",
        ),
        (
            "a prune's deletion after the add returned still does",
            "`PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`",
        ),
        (
            "a host agent's own prune is one starter of it",
            "`PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD`",
        ),
        (
            "an attempt's Git error is the pipeline's and ends the command resumably instead",
            "its Git error is the pipeline's, and the command ends resumably",
        ),
    ] {
        assert!(
            arm.contains(pin),
            "the verification's Git arm must state that {proposition}; looked for {pin:?} \
             in:\n{arm}"
        );
    }

    let torn = super::verified(
        Err(JudgeError::Other(crate::error::UpstrokeError::Git {
            message: "git worktree list --porcelain -z failed: fatal: failed to read \
                      .git/worktrees/half-written/commondir: Success"
                .to_owned(),
        })),
        Vec::new(),
        SequenceId(1),
    );
    match torn {
        Ok(Verified::Unavailable { detail, .. }) => assert!(
            detail.contains("half-written/commondir"),
            "the outage carries what Git said: {detail}"
        ),
        Ok(Verified::Judged(_)) => {
            panic!("the mapping the notes state: a Git error in a verification is not judged")
        }
        Err(error) => panic!(
            "the mapping the notes state: a Git error in a verification's judgement is an outage \
             of its sequence, not an error that ends the command: {error}"
        ),
    }

    let refused = super::verified(
        Err(JudgeError::Other(
            crate::error::UpstrokeError::RegistryRefused {
                message: "the worktree registry kept this access from completing until its \
                          deadline"
                    .to_owned(),
            },
        )),
        Vec::new(),
        SequenceId(1),
    );
    match refused {
        Err(crate::error::UpstrokeError::RegistryRefused { .. }) => {}
        Ok(Verified::Unavailable { detail, .. }) => panic!(
            "the mapping the notes state: a registry refusal is never an outage of the sequence, \
             and this one was mapped to one: {detail}"
        ),
        Ok(Verified::Judged(_)) => panic!("a registry refusal is not a verdict"),
        Err(error) => panic!("the registry refusal is passed on as it is, not as {error:?}"),
    }
}

#[test]
fn a_cancelled_verification_is_never_classified_as_an_outage() {
    use crate::engine::topology::attempt::JudgeError;
    use crate::engine::topology::integrate::Verified;
    use crate::error::ProcessFate;
    use crate::runner::RunnerError;
    use crate::topology::events::SequenceId;

    let invocation =
        crate::engine::topology::identity::SequenceIdentities::new(SequenceId(3)).gate(0, 0);
    let cancelled = RunnerError::cancelled(&invocation, ProcessFate::Gone);
    let error = super::verified(
        Err(JudgeError::Runner(cancelled)),
        Vec::new(),
        SequenceId(3),
    )
    .err()
    .expect("a cancelled verification is an error, not a verdict");
    assert!(
        matches!(
            error,
            crate::error::UpstrokeError::Runner {
                fate: ProcessFate::Gone,
                ..
            }
        ),
        "{error:?}"
    );

    let gone = RunnerError::gone(
        &invocation,
        crate::error::UpstrokeError::Refused {
            message: "the runtime lost the process".to_owned(),
        },
    );
    assert!(
        matches!(
            super::verified(Err(JudgeError::Runner(gone)), Vec::new(), SequenceId(3)),
            Ok(Verified::Unavailable { .. })
        ),
        "the control: a process the Runner lost, uncancelled, is still an outage"
    );
}

mod scaffold_runner {
    use std::task::{Context, Waker};
    use std::time::Duration;

    use super::super::super::scaffold::{Ending, ProbeFailure, RecordingRunner};
    use crate::agent::ProcessOutput;
    use crate::error::ProcessFate;
    use crate::gates::ShellKind;
    use crate::runner::invocation::AttemptRole;
    use crate::runner::{
        AgentId, Cancellation, InvocationId, ProbeTarget, Runner, RunnerCall, RunnerRequest,
    };
    use crate::topology::events::{
        AttemptNumber, GenerationId, ImageIdentity, RunnerContract, RunnerKind, RunnerPolicy,
    };
    use crate::topology::registry::TaskKey;

    fn gate(ordinal: u32) -> RunnerRequest {
        crate::runner::gate_request(
            ShellKind::native().spec("exit 0"),
            std::env::temp_dir(),
            Duration::from_secs(5),
            InvocationId::attempt(
                TaskKey(0),
                GenerationId(0),
                AttemptNumber(1),
                AttemptRole::Gate(ordinal),
                0,
            ),
        )
    }

    fn output(code: i32) -> ProcessOutput {
        ProcessOutput {
            code: Some(code),
            stdout: String::new(),
            stderr: String::new(),
            duration: Duration::from_millis(1),
            timed_out: false,
            output_limited: false,
        }
    }

    #[test]
    fn the_scaffold_runner_holds_each_invocation_until_the_test_completes_it_in_any_order() {
        let runner = RecordingRunner::new();
        runner.hold();
        let requests: Vec<RunnerRequest> = (0..3).map(gate).collect();
        let codes = std::thread::scope(|scope| {
            let drivers: Vec<_> = requests
                .iter()
                .map(|request| scope.spawn(|| runner.run_blocking(request)))
                .collect();
            let waiting = runner.await_waiting(3, Duration::from_secs(30));
            assert_eq!(
                waiting.len(),
                3,
                "every invocation started and waits: {waiting:?}"
            );
            for (request, code) in requests.iter().zip([10, 11, 12]).rev() {
                runner
                    .complete(&request.invocation, Ok(output(code)))
                    .expect("a held invocation takes its completion");
            }
            drivers
                .into_iter()
                .map(|driver| {
                    driver
                        .join()
                        .expect("a driving thread")
                        .expect("completed")
                        .code
                })
                .collect::<Vec<_>>()
        });
        assert_eq!(
            codes,
            vec![Some(10), Some(11), Some(12)],
            "each invocation got its own completion, whatever the order they were delivered in"
        );
        assert_eq!(
            runner.endings(),
            requests
                .iter()
                .rev()
                .map(|request| (request.invocation.clone(), Ending::Completed))
                .collect::<Vec<_>>(),
            "the endings are recorded in the order the test delivered them"
        );
        let ran: Vec<InvocationId> = runner
            .ran()
            .into_iter()
            .map(|ran| ran.request.invocation)
            .collect();
        let mut expected: Vec<InvocationId> = requests
            .iter()
            .map(|request| request.invocation.clone())
            .collect();
        let mut recorded = ran.clone();
        recorded.sort_by_key(InvocationId::render);
        expected.sort_by_key(InvocationId::render);
        assert_eq!(
            recorded, expected,
            "every request is recorded with its identity"
        );
        assert!(runner.waiting().is_empty());
    }

    #[test]
    fn the_scaffold_runner_refuses_and_counts_a_second_completion_and_an_unknown_one() {
        let runner = RecordingRunner::new();
        runner.hold();
        let request = gate(0);
        let outcome = std::thread::scope(|scope| {
            let driver = scope.spawn(|| runner.run_blocking(&request));
            assert_eq!(runner.await_waiting(1, Duration::from_secs(30)).len(), 1);
            runner
                .complete(&request.invocation, Ok(output(0)))
                .expect("the first completion");
            let second = runner.complete(&request.invocation, Ok(output(1)));
            let outcome = driver.join().expect("the driving thread");
            (second, outcome)
        });
        let (second, finished) = outcome;
        assert!(
            second.is_err(),
            "a second completion of one invocation is refused"
        );
        assert_eq!(
            finished.expect("completed").code,
            Some(0),
            "the first completion stood"
        );
        assert!(
            runner.complete(&gate(7).invocation, Ok(output(0))).is_err(),
            "a completion for an invocation this runner never held is refused"
        );
        assert_eq!(runner.refused_completions(), 2, "both refusals are counted");
        assert_eq!(runner.endings().len(), 1, "one invocation, one ending");
    }

    #[test]
    fn the_scaffold_runner_ends_a_cancelled_or_dropped_invocation_exactly_once() {
        let runner = RecordingRunner::new();
        runner.hold();
        let cancelled = gate(0);
        let cancellation = Cancellation::new();
        let outcome = std::thread::scope(|scope| {
            let driver = scope.spawn(|| {
                runner.run_blocking_with(&cancelled, RunnerCall::new(cancellation.clone()))
            });
            assert_eq!(runner.await_waiting(1, Duration::from_secs(30)).len(), 1);
            cancellation.cancel();
            driver.join().expect("the driving thread")
        });
        let error = outcome.expect_err("a cancelled invocation reports an error");
        assert!(error.is_cancelled(), "{error}");
        assert_eq!(error.fate, ProcessFate::Gone);
        assert!(
            runner
                .complete(&cancelled.invocation, Ok(output(0)))
                .is_err(),
            "a completion after the cancellation finds nothing to complete"
        );

        let dropped = gate(1);
        {
            let mut future = runner.run(&dropped, RunnerCall::default());
            let mut context = Context::from_waker(Waker::noop());
            assert!(future.as_mut().poll(&mut context).is_pending(), "held");
            assert_eq!(runner.waiting(), vec![dropped.invocation.clone()]);
        }
        assert!(
            runner.waiting().is_empty(),
            "dropping the future released the hold"
        );

        let unstarted = gate(2);
        let before = Cancellation::new();
        before.cancel();
        let error = runner
            .run_blocking_with(&unstarted, RunnerCall::new(before))
            .expect_err("a call cancelled before it starts reports an error");
        assert!(error.is_cancelled(), "{error}");
        assert_eq!(error.fate, ProcessFate::NeverStarted);
        assert!(
            !runner
                .ran()
                .iter()
                .any(|ran| ran.invocation == unstarted.invocation),
            "no process of it started"
        );
        assert_eq!(
            runner.endings(),
            vec![
                (cancelled.invocation.clone(), Ending::Cancelled),
                (dropped.invocation.clone(), Ending::Abandoned),
                (unstarted.invocation.clone(), Ending::CancelledBeforeStart),
            ]
        );
    }

    #[test]
    fn a_late_call_waits_at_the_door_until_admitted_and_a_barred_one_until_cancelled() {
        let runner = RecordingRunner::new();
        runner.hold();
        runner.enter_late();
        let late = gate(0);
        let barred = gate(1);
        runner.bar(barred.invocation.clone());
        let cancellation = Cancellation::new();
        let (admitted, cancelled) = std::thread::scope(|scope| {
            let admitted = scope.spawn(|| runner.run_blocking(&late));
            let cancelled = scope
                .spawn(|| runner.run_blocking_with(&barred, RunnerCall::new(cancellation.clone())));
            assert!(
                runner.admit(&barred.invocation, Duration::from_secs(30)),
                "a barred call is inside the runner once it waits at its door"
            );
            assert!(
                !runner.inside(&late.invocation) && runner.ran().is_empty(),
                "a late call starts only when it is admitted"
            );
            assert!(runner.admit(&late.invocation, Duration::from_secs(30)));
            assert_eq!(
                runner.waiting(),
                vec![late.invocation.clone()],
                "admitting a late call starts it; the barred one still waits at its door"
            );
            runner
                .complete(&late.invocation, Ok(output(0)))
                .expect("the admitted call is held");
            cancellation.cancel();
            (
                admitted.join().expect("the late call's thread"),
                cancelled.join().expect("the barred call's thread"),
            )
        });
        assert_eq!(admitted.expect("completed").code, Some(0));
        let error = cancelled.expect_err("the barred call was cancelled");
        assert!(error.is_cancelled(), "{error}");
        assert_eq!(error.fate, ProcessFate::NeverStarted);
        let started: Vec<InvocationId> =
            runner.ran().into_iter().map(|ran| ran.invocation).collect();
        assert_eq!(started, vec![late.invocation.clone()]);
        assert_eq!(
            runner.endings(),
            vec![
                (late.invocation.clone(), Ending::Completed),
                (barred.invocation.clone(), Ending::CancelledBeforeStart),
            ]
        );
    }

    #[test]
    fn the_scaffold_runner_records_the_policy_and_image_each_invocation_ran_under() {
        let host = RecordingRunner::new();
        host.run_blocking(&gate(0)).expect("runs");
        let recorded = host.ran();
        assert_eq!(recorded.len(), 1);
        for ran in &recorded {
            assert_eq!(ran.policy, crate::runner::policy::host_policy());
            assert_eq!(ran.image_id, None, "a host boundary has no image");
        }

        let declared = RunnerPolicy {
            kind: RunnerKind::Container,
            policy: RunnerContract::ContainerV1,
            image: Some(ImageIdentity {
                reference: "upstroke/agents:2026-09".to_owned(),
                id: "sha256:0f0e0d0c".to_owned(),
                digest: None,
            }),
            credential_volumes: None,
            mounts: Vec::new(),
        };
        let container = RecordingRunner::new().declaring(declared.clone());
        container.run_blocking(&gate(1)).expect("runs");
        let recorded = container.ran();
        assert_eq!(recorded.len(), 1);
        for ran in &recorded {
            assert_eq!(ran.policy, declared);
            assert_eq!(ran.image_id.as_deref(), Some("sha256:0f0e0d0c"));
            assert_eq!(ran.request.invocation, gate(1).invocation);
        }
    }

    #[test]
    fn the_scaffold_runner_fails_a_shell_or_agent_probe_on_demand() {
        let runner = RecordingRunner::new();
        runner.fail_probe(
            ProbeTarget::Shell,
            ProbeFailure::Exit {
                code: 127,
                stderr: "sh: not found".to_owned(),
            },
        );
        let refused = crate::runner::host::run_shell_probe(
            &runner,
            ShellKind::native(),
            std::env::temp_dir(),
            InvocationId::probe(ProbeTarget::Shell, 0).expect("a probe identity"),
        )
        .expect_err("the shell probe fails when told to");
        assert!(refused.to_string().contains("127"), "{refused}");
        crate::runner::host::run_shell_probe(
            &runner,
            ShellKind::native(),
            std::env::temp_dir(),
            InvocationId::probe(ProbeTarget::Shell, 1).expect("a probe identity"),
        )
        .expect("a failure is used once; the next shell probe passes");

        let agent = AgentId::new(crate::agent::claude::ADAPTER_ID);
        runner.fail_probe(
            ProbeTarget::Agent(agent.clone()),
            ProbeFailure::NeverStarted,
        );
        let probe = crate::agent::probe_request(
            crate::agent::claude::ADAPTER_ID,
            ShellKind::native().spec("exit 0"),
            0,
            Duration::from_secs(5),
        )
        .expect("a probe request");
        let error = runner
            .run_blocking(&probe)
            .expect_err("the agent probe fails when told to");
        assert_eq!(error.fate, ProcessFate::NeverStarted);
        assert!(!error.is_cancelled());
        assert_eq!(
            runner.ran().len(),
            3,
            "failed probes are recorded like any request"
        );
    }
}

/// J2, `PR249-ANSWER-TEXT-NOT-CARRIED`: the brief takes a person's words from
/// `question_answered`, live and on replay alike. Words that name none of the question's options
/// become an instruction from a person, which is also the review's decision. A chosen option, a
/// spend approval, a decline, and an older record with no words add nothing.
#[test]
fn the_brief_takes_an_answers_words_from_the_log_live_and_on_replay() {
    use crate::topology::events::{QuestionAnswered4, QuestionRaised4};

    let words = "Use UNIQUE_ZEBRA_FORMAT_739 for the widget.";
    let ask = |id: &str, kind: QuestionKind| FrozenQuestion {
        id: QuestionId(id.to_owned()),
        key: TaskKey(0),
        kind,
        context: "a person is asked".to_owned(),
        options: crate::engine::coordinator::topology_question_options(kind),
    };
    let answered = |id: &str, answer: Answer4, text: Option<&str>| QuestionAnswered4 {
        key: TaskKey(0),
        question: QuestionId(id.to_owned()),
        answer,
        via: "event-log".to_owned(),
        text: text.map(str::to_owned),
    };
    let chosen = Answer4::Answered {
        option_index: 0,
        binding_override: None,
    };
    let declined = Answer4::Declined {
        decline_halts_run: false,
    };
    let options = crate::engine::coordinator::topology_question_options(QuestionKind::Clarify);
    let retry = &options[0];
    let cases = [
        (
            ask("q-words", QuestionKind::Clarify),
            answered("q-words", chosen.clone(), Some(words)),
        ),
        (
            ask("q-option", QuestionKind::Clarify),
            answered("q-option", chosen.clone(), Some(retry)),
        ),
        (
            ask("q-spend", QuestionKind::ApproveSpend),
            answered("q-spend", chosen.clone(), Some("spend it")),
        ),
        (
            ask("q-older", QuestionKind::Unblock),
            answered("q-older", chosen, None),
        ),
        (
            ask("q-declined", QuestionKind::Continue),
            answered("q-declined", declined, None),
        ),
    ];

    let mut live = Brief::new();
    let mut events = Vec::new();
    for (question, answer) in &cases {
        live.record_answer(question.kind, &question.options, answer);
        for body in [
            TopologyEventBody::QuestionRaised {
                data: QuestionRaised4 {
                    question: question.clone(),
                },
            },
            TopologyEventBody::QuestionAnswered {
                data: answer.clone(),
            },
        ] {
            events.push(TopologyEvent {
                ts: "2026-11-03T00:00:00.000Z".to_owned(),
                body,
            });
        }
    }
    let replayed = Brief::replay(&events);
    assert_eq!(replayed, live, "the replay reaches the live brief");
    let lines = replayed.lines(TaskKey(0));
    let [line] = lines.as_slice() else {
        panic!("one instruction from a person: {lines:?}");
    };
    assert!(line.human, "{line:?}");
    assert_eq!(line.detail.as_deref(), Some(words));
    assert_eq!(replayed.decisions(TaskKey(0)), [words]);
}

#[test]
fn a_rate_limited_worker_naming_a_reset_writes_one_pool_exhausted_before_its_attempt_finished() {
    use std::sync::Arc;

    use crate::engine::topology::scaffold::{
        RecordingRunner, ScaffoldAdapters, Wide, WidePlans, WideTask, exited,
    };
    use crate::runner::InvocationId;
    use crate::runner::invocation::AttemptRole;

    struct At(&'static str);

    impl TimeSource for At {
        fn now_rfc3339(&self) -> String {
            self.0.to_owned()
        }
    }

    const LIMIT: &str = "You've hit your 5-hour limit · resets 5:30am (UTC)";
    let runner = RecordingRunner::new().answering(Box::new(|request| {
        Ok(match &request.invocation {
            InvocationId::Attempt {
                role: AttemptRole::Worker,
                ..
            } => crate::agent::ProcessOutput {
                stderr: LIMIT.to_owned(),
                ..exited(1, String::new())
            },
            _ => exited(0, String::new()),
        })
    }));
    let mut wide = Wide::started_with(
        "pool-exhausted-before-settlement",
        &[WideTask::independent("alpha")],
        1,
        WidePlans {
            pool: Some("claude-max".to_owned()),
            ..WidePlans::default()
        },
        runner,
    );
    wide.env.adapters = Arc::new(ScaffoldAdapters::rate_limiting());
    let clock = At("2026-11-11T03:00:00Z");
    let seams = RunSeams {
        clock: &clock,
        ..wide.env.seams()
    };
    let mut hooks = wide.env.hooks();
    let progress = wide
        .run
        .step(&seams, &mut hooks)
        .expect("the rate-limited attempt settles");
    assert!(
        matches!(
            progress,
            Progress::Settled {
                accepted: false,
                ..
            }
        ),
        "{progress:?}"
    );

    let events = wide.env.durable_events();
    let signals: Vec<(usize, &crate::events::PoolExhausted)> = events
        .iter()
        .enumerate()
        .filter_map(|(at, event)| match &event.body {
            TopologyEventBody::PoolExhausted { data } => Some((at, data)),
            _ => None,
        })
        .collect();
    let [(signalled, signal)] = signals.as_slice() else {
        panic!("exactly one `pool_exhausted`: {signals:?}");
    };
    assert_eq!(signal.pool, "claude-max");
    assert_eq!(signal.agent, crate::agent::claude::ADAPTER_ID);
    assert_eq!(
        signal.reset_at.as_deref(),
        Some("2026-11-11T05:30:00Z"),
        "the reset the limit message names, read against the run's clock: {signal:?}"
    );
    let finished = events
        .iter()
        .position(|event| {
            matches!(
                &event.body,
                TopologyEventBody::AttemptFinished { data }
                    if data.key == TaskKey(0) && data.attempt == AttemptNumber(1)
            )
        })
        .expect("the attempt settled with `attempt_finished`");
    assert!(
        *signalled < finished,
        "`pool_exhausted` at {signalled} must come before the attempt's `attempt_finished` at \
         {finished}: {:?}",
        events
            .iter()
            .map(|event| event.body.kind())
            .collect::<Vec<_>>()
    );
}

const FIVE_HOUR_LIMIT: &str = "You've hit your 5-hour limit · resets 5:30am (UTC)";
const WEEKLY_LIMIT: &str = "You've hit your weekly limit · resets Nov 14, 5pm (UTC)";

struct RunClock;

impl TimeSource for RunClock {
    fn now_rfc3339(&self) -> String {
        "2026-11-11T03:00:00Z".to_owned()
    }
}

fn on_claude_max(
    tag: &str,
    tasks: &[crate::engine::topology::scaffold::WideTask],
    limited: &[(TaskKey, GenerationId, &'static str)],
    failing_gates: &[(u32, u32)],
) -> crate::engine::topology::scaffold::Wide {
    use crate::engine::topology::scaffold::{Wide, WidePlans};

    Wide::started_with(
        tag,
        tasks,
        1,
        WidePlans {
            pool: Some("claude-max".to_owned()),
            ..WidePlans::default()
        },
        limited_runner(tasks, limited, failing_gates),
    )
}

fn limited_runner(
    tasks: &[crate::engine::topology::scaffold::WideTask],
    limited: &[(TaskKey, GenerationId, &'static str)],
    failing_gates: &[(u32, u32)],
) -> crate::engine::topology::scaffold::RecordingRunner {
    use crate::engine::topology::scaffold::{RecordingRunner, exited, wide_responder};
    use crate::runner::InvocationId;
    use crate::runner::invocation::AttemptRole;

    let otherwise = wide_responder(tasks, failing_gates);
    let limited = limited.to_vec();
    RecordingRunner::new().answering(Box::new(move |request| {
        if let InvocationId::Attempt {
            key,
            generation,
            role: AttemptRole::Worker,
            ..
        } = &request.invocation
        {
            let limit = limited
                .iter()
                .find(|(on, of, _)| on == key && of == generation);
            if let Some((_, _, limit)) = limit {
                return Ok(crate::agent::ProcessOutput {
                    stderr: (*limit).to_owned(),
                    ..exited(1, String::new())
                });
            }
        }
        otherwise(request)
    }))
}

fn step_answered_by(
    wide: &mut crate::engine::topology::scaffold::Wide,
    adapters: crate::engine::topology::scaffold::ScaffoldAdapters,
    hooks: &mut dyn TopologyHooks,
) -> Progress {
    wide.env.adapters = std::sync::Arc::new(adapters);
    let seams = RunSeams {
        clock: &RunClock,
        ..wide.env.seams()
    };
    wide.run.step(&seams, hooks).expect("a step")
}

fn rate_limited_step_past_any_wait(
    wide: &mut crate::engine::topology::scaffold::Wide,
    hooks: &mut dyn TopologyHooks,
) -> Progress {
    use crate::engine::topology::scaffold::ScaffoldAdapters;

    for _ in 0..64 {
        let progress = step_answered_by(wide, ScaffoldAdapters::rate_limiting(), hooks);
        if !matches!(progress, Progress::Waiting { .. }) {
            return progress;
        }
    }
    panic!("the run never came back from its wait toward the reset");
}

fn pools_exhausted(
    wide: &crate::engine::topology::scaffold::Wide,
) -> Vec<(String, Option<String>)> {
    wide.env
        .durable_events()
        .into_iter()
        .filter_map(|event| match event.body {
            TopologyEventBody::PoolExhausted { data } => Some((data.pool, data.reset_at)),
            _ => None,
        })
        .collect()
}

fn failures_of(
    wide: &crate::engine::topology::scaffold::Wide,
    key: TaskKey,
) -> Vec<Option<crate::ladder::FailureKind>> {
    wide.env
        .durable_events()
        .into_iter()
        .filter_map(|event| match event.body {
            TopologyEventBody::AttemptFinished { data } if data.key == key => {
                Some(data.record.failure.map(|failure| failure.kind))
            }
            _ => None,
        })
        .collect()
}

fn signal(reset_at: &str) -> (String, Option<String>) {
    ("claude-max".to_owned(), Some(reset_at.to_owned()))
}

#[test]
fn a_second_rate_limit_naming_the_same_reset_writes_no_second_pool_exhausted() {
    use crate::engine::topology::scaffold::WideTask;
    use crate::ladder::FailureKind;

    let mut wide = on_claude_max(
        "pool-exhausted-same-reset",
        &[WideTask::independent("alpha")],
        &[
            (TaskKey(0), GenerationId(0), FIVE_HOUR_LIMIT),
            (TaskKey(0), GenerationId(1), FIVE_HOUR_LIMIT),
        ],
        &[],
    );
    let mut hooks = wide.env.hooks();
    let progress: Vec<Progress> = (0..3)
        .map(|_| rate_limited_step_past_any_wait(&mut wide, &mut hooks))
        .collect();
    assert!(
        matches!(
            progress.as_slice(),
            [
                Progress::Settled {
                    accepted: false,
                    ..
                },
                Progress::Waited { .. },
                Progress::Settled {
                    accepted: false,
                    ..
                },
            ]
        ),
        "deferred, woken, and redispatched into a second limit: {progress:?}"
    );
    assert_eq!(
        failures_of(&wide, TaskKey(0)),
        [
            Some(FailureKind::RateLimited),
            Some(FailureKind::RateLimited)
        ]
    );
    assert_eq!(
        pools_exhausted(&wide),
        [signal("2026-11-11T05:30:00Z")],
        "the second limit names the reset the first one recorded"
    );
}

#[test]
fn a_second_rate_limit_naming_a_different_reset_writes_one_more_pool_exhausted() {
    use crate::engine::topology::scaffold::WideTask;
    use crate::ladder::FailureKind;

    let mut wide = on_claude_max(
        "pool-exhausted-new-reset",
        &[WideTask::independent("alpha")],
        &[
            (TaskKey(0), GenerationId(0), FIVE_HOUR_LIMIT),
            (TaskKey(0), GenerationId(1), WEEKLY_LIMIT),
        ],
        &[],
    );
    let mut hooks = wide.env.hooks();
    let progress: Vec<Progress> = (0..3)
        .map(|_| rate_limited_step_past_any_wait(&mut wide, &mut hooks))
        .collect();
    assert!(
        matches!(
            progress.as_slice(),
            [
                Progress::Settled {
                    accepted: false,
                    ..
                },
                Progress::Waited { .. },
                Progress::Settled {
                    accepted: false,
                    ..
                },
            ]
        ),
        "deferred, woken, and redispatched into a second limit: {progress:?}"
    );
    assert_eq!(
        failures_of(&wide, TaskKey(0)),
        [
            Some(FailureKind::RateLimited),
            Some(FailureKind::RateLimited)
        ]
    );
    assert_eq!(
        pools_exhausted(&wide),
        [
            signal("2026-11-11T05:30:00Z"),
            signal("2026-11-14T17:00:00Z")
        ],
        "the weekly limit moves the pool's reset, so it is recorded"
    );
}

#[test]
fn a_rate_limit_after_an_attempt_the_pool_served_writes_pool_exhausted_again() {
    use crate::engine::topology::scaffold::{ScaffoldAdapters, WideTask};

    let mut wide = on_claude_max(
        "pool-exhausted-after-serving",
        &[
            WideTask::independent("alpha"),
            WideTask::independent("beta"),
        ],
        &[
            (TaskKey(0), GenerationId(0), FIVE_HOUR_LIMIT),
            (TaskKey(0), GenerationId(1), FIVE_HOUR_LIMIT),
        ],
        &[],
    );
    let mut hooks = wide.env.hooks();
    let progress: Vec<Progress> = [
        ScaffoldAdapters::rate_limiting(),
        ScaffoldAdapters::new(),
        ScaffoldAdapters::new(),
        ScaffoldAdapters::new(),
        ScaffoldAdapters::rate_limiting(),
    ]
    .into_iter()
    .map(|adapters| step_answered_by(&mut wide, adapters, &mut hooks))
    .collect();
    assert!(
        matches!(
            progress.as_slice(),
            [
                Progress::Settled {
                    key: TaskKey(0),
                    accepted: false,
                    ..
                },
                Progress::Settled {
                    key: TaskKey(1),
                    accepted: true,
                    ..
                },
                Progress::Integrated {
                    key: TaskKey(1),
                    ..
                },
                Progress::Waited { .. },
                Progress::Settled {
                    key: TaskKey(0),
                    accepted: false,
                    ..
                },
            ]
        ),
        "alpha is limited, beta is served and merged on the same pool, and alpha is limited \
         again: {progress:?}"
    );
    assert_eq!(
        pools_exhausted(&wide),
        [
            signal("2026-11-11T05:30:00Z"),
            signal("2026-11-11T05:30:00Z")
        ],
        "beta's accepted attempt proved the pool serving, so the same reset is recorded again"
    );
}

#[test]
fn a_failure_that_is_not_rate_limited_writes_no_pool_exhausted() {
    use crate::engine::topology::scaffold::{ScaffoldAdapters, WideTask};
    use crate::ladder::FailureKind;

    for (tag, adapters, limited, failing_gates, kind) in [
        (
            "pool-exhausted-agent-error",
            ScaffoldAdapters::erroring(),
            &[(TaskKey(0), GenerationId(0), FIVE_HOUR_LIMIT)][..],
            &[][..],
            FailureKind::AgentError,
        ),
        (
            "pool-exhausted-gate-failed",
            ScaffoldAdapters::new(),
            &[][..],
            &[(0, 1)][..],
            FailureKind::GateFailed,
        ),
    ] {
        let mut wide = on_claude_max(
            tag,
            &[WideTask::independent("alpha")],
            limited,
            failing_gates,
        );
        let mut hooks = wide.env.hooks();
        let progress = step_answered_by(&mut wide, adapters, &mut hooks);
        assert!(
            matches!(
                progress,
                Progress::Settled {
                    accepted: false,
                    ..
                }
            ),
            "{tag}: {progress:?}"
        );
        assert_eq!(failures_of(&wide, TaskKey(0)), [Some(kind)], "{tag}");
        assert_eq!(pools_exhausted(&wide), [], "{tag}");
    }
}

fn durable_on_claude_max(
    tag: &str,
    tasks: &[crate::engine::topology::scaffold::WideTask],
    limited: &[(TaskKey, GenerationId, &'static str)],
) -> crate::engine::topology::scaffold::Wide {
    use crate::engine::topology::scaffold::{Wide, WidePlans};

    Wide::durable(
        tag,
        tasks,
        1,
        WidePlans {
            pool: Some("claude-max".to_owned()),
            ..WidePlans::default()
        },
        limited_runner(tasks, limited, &[]),
    )
}

fn resumed_limiting(
    wide: crate::engine::topology::scaffold::Wide,
    tasks: &[crate::engine::topology::scaffold::WideTask],
    limited: &[(TaskKey, GenerationId, &'static str)],
) -> crate::engine::topology::scaffold::Wide {
    let (_, resumed) = wide
        .resume(
            "inc-2",
            limited_runner(tasks, limited, &[]),
            Ceiling::unlimited(),
        )
        .expect("the next process resumes the run");
    resumed
}

#[test]
fn a_resumed_run_writes_pool_exhausted_again_only_when_the_limit_names_another_reset() {
    use crate::engine::topology::scaffold::{ScaffoldAdapters, WideTask};
    use crate::ladder::FailureKind;

    let tasks = [WideTask::independent("alpha")];
    for (tag, next_limit, expected) in [
        (
            "resumed-pool-exhausted-same-reset",
            FIVE_HOUR_LIMIT,
            vec![signal("2026-11-11T05:30:00Z")],
        ),
        (
            "resumed-pool-exhausted-new-reset",
            WEEKLY_LIMIT,
            vec![
                signal("2026-11-11T05:30:00Z"),
                signal("2026-11-14T17:00:00Z"),
            ],
        ),
    ] {
        let mut wide = durable_on_claude_max(
            tag,
            &tasks,
            &[(TaskKey(0), GenerationId(0), FIVE_HOUR_LIMIT)],
        );
        let mut hooks = wide.env.hooks();
        let first = step_answered_by(&mut wide, ScaffoldAdapters::rate_limiting(), &mut hooks);
        assert!(
            matches!(
                first,
                Progress::Settled {
                    accepted: false,
                    ..
                }
            ),
            "{tag}: {first:?}"
        );
        assert_eq!(
            pools_exhausted(&wide),
            [signal("2026-11-11T05:30:00Z")],
            "{tag}: the first process recorded the five-hour reset"
        );

        let mut resumed =
            resumed_limiting(wide, &tasks, &[(TaskKey(0), GenerationId(1), next_limit)]);
        let mut hooks = resumed.env.hooks();
        let progress =
            step_answered_by(&mut resumed, ScaffoldAdapters::rate_limiting(), &mut hooks);
        assert!(
            matches!(
                progress,
                Progress::Settled {
                    key: TaskKey(0),
                    accepted: false,
                    ..
                }
            ),
            "{tag}: the resume wakes the deferred task, and its next generation is limited \
             again: {progress:?}"
        );
        assert_eq!(
            failures_of(&resumed, TaskKey(0)),
            [
                Some(FailureKind::RateLimited),
                Some(FailureKind::RateLimited)
            ],
            "{tag}"
        );
        assert_eq!(
            pools_exhausted(&resumed),
            expected,
            "{tag}: the resumed run remembers the reset its log recorded"
        );
    }
}

struct CutAfterPoolExhausted {
    inner: crate::engine::topology::seams::HarnessTopologyHooks,
    harness: std::sync::Arc<std::sync::Mutex<crate::topology::effects::HookHarness>>,
}

impl TopologyHooks for CutAfterPoolExhausted {
    fn effects(&mut self) -> &mut dyn crate::workspace_manager::EffectHooks {
        self.inner.effects()
    }

    fn rundir(&mut self) -> &mut dyn crate::rundir::RunDirHooks {
        self.inner.rundir()
    }

    fn events(&mut self) -> &mut dyn crate::events::log::EventHooks {
        self.inner.events()
    }

    fn container(&mut self) -> &mut dyn crate::runner::container::ContainerHooks {
        self.inner.container()
    }

    fn spawn(&mut self) -> &mut dyn crate::agent::proc::SpawnHooks {
        self.inner.spawn()
    }

    fn folded(&mut self, fold: &TopologyFold, events: &[TopologyEvent]) {
        self.inner.folded(fold, events);
        if matches!(
            events.last().map(|event| &event.body),
            Some(TopologyEventBody::PoolExhausted { .. })
        ) {
            self.harness
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .arm(
                    crate::topology::effects::EffectSiteId::Event(
                        crate::topology::effects::EventSite::Append,
                    ),
                    crate::topology::effects::SubEffectPoint::Written,
                    crate::topology::effects::InjectionMode::ErrorReturn,
                )
                .expect("the Event append supports an error return while it writes");
        }
    }
}

#[test]
fn a_run_whose_log_ends_with_pool_exhausted_resumes_with_that_pool_exhausted_at_that_reset() {
    use crate::engine::topology::scaffold::{ScaffoldAdapters, WideTask};

    let tasks = [WideTask::independent("alpha")];
    let mut wide = durable_on_claude_max(
        "resumed-pool-exhausted-cut",
        &tasks,
        &[(TaskKey(0), GenerationId(0), FIVE_HOUR_LIMIT)],
    );
    wide.env.adapters = std::sync::Arc::new(ScaffoldAdapters::rate_limiting());
    let mut hooks = CutAfterPoolExhausted {
        inner: wide.env.hooks(),
        harness: std::sync::Arc::clone(&wide.env.harness),
    };
    let seams = RunSeams {
        clock: &RunClock,
        ..wide.env.seams()
    };
    let error = wide
        .run
        .step(&seams, &mut hooks)
        .expect_err("the settlement's append fails after the signal's");
    let kinds: Vec<&str> = wide
        .env
        .durable_events()
        .iter()
        .map(|event| event.body.kind())
        .collect();
    assert_eq!(
        kinds.last(),
        Some(&"pool_exhausted"),
        "the log ends with the signal and no settlement after it: {kinds:?} ({error})"
    );
    assert!(!kinds.contains(&"attempt_finished"), "{kinds:?}");
    assert_eq!(pools_exhausted(&wide), [signal("2026-11-11T05:30:00Z")]);

    let mut resumed = resumed_limiting(
        wide,
        &tasks,
        &[
            (TaskKey(0), GenerationId(0), FIVE_HOUR_LIMIT),
            (TaskKey(0), GenerationId(1), FIVE_HOUR_LIMIT),
        ],
    );
    let mut hooks = resumed.env.hooks();
    let mut progress = Vec::new();
    while !matches!(progress.last(), Some(Progress::Settled { .. })) && progress.len() < 4 {
        progress.push(step_answered_by(
            &mut resumed,
            ScaffoldAdapters::rate_limiting(),
            &mut hooks,
        ));
    }
    assert!(
        matches!(
            progress.last(),
            Some(Progress::Settled {
                key: TaskKey(0),
                accepted: false,
                ..
            })
        ),
        "the resumed run runs alpha again into the same limit: {progress:?}"
    );
    assert_eq!(
        failures_of(&resumed, TaskKey(0)),
        [Some(crate::ladder::FailureKind::RateLimited)],
        "{progress:?}"
    );
    assert_eq!(
        pools_exhausted(&resumed),
        [signal("2026-11-11T05:30:00Z")],
        "the resumed run holds the pool exhausted at the reset the cut log recorded, so the same \
         limit writes no second signal: {progress:?}"
    );
}

#[derive(Default)]
struct RecordingSleeper(std::cell::RefCell<Vec<std::time::Duration>>);

impl RecordingSleeper {
    fn slept(&self) -> Vec<std::time::Duration> {
        self.0.borrow().clone()
    }
}

impl Sleeper for RecordingSleeper {
    fn sleep(&self, duration: std::time::Duration) {
        self.0.borrow_mut().push(duration);
    }
}

fn defer_waits(wide: &crate::engine::topology::scaffold::Wide) -> Vec<(u64, u32)> {
    wide.env
        .durable_events()
        .into_iter()
        .filter_map(|event| match event.body {
            TopologyEventBody::DeferWaitElapsed { data } => Some((data.waited_ms, data.round)),
            _ => None,
        })
        .collect()
}

#[derive(Debug)]
struct Stepped {
    progress: Progress,
    defer_waits: Vec<(u64, u32)>,
    slices: usize,
}

fn stepped_until_waited(
    wide: &mut crate::engine::topology::scaffold::Wide,
    clock: &dyn TimeSource,
    sleeper: &RecordingSleeper,
    answers: Option<&dyn crate::interaction::AnswerSource>,
    hooks: &mut dyn TopologyHooks,
) -> Vec<Stepped> {
    let mut steps: Vec<Stepped> = Vec::new();
    while !matches!(
        steps.last(),
        Some(Stepped {
            progress: Progress::Waited { .. },
            ..
        })
    ) {
        assert!(
            steps.len() < 64,
            "the run never came back from its back-off: {steps:?}"
        );
        let defaults = wide.env.seams();
        let seams = RunSeams {
            clock,
            sleeper,
            answers: answers.unwrap_or(defaults.answers),
            ..defaults
        };
        let progress = wide.run.step(&seams, hooks).expect("a step");
        steps.push(Stepped {
            progress,
            defer_waits: defer_waits(wide),
            slices: sleeper.slept().len(),
        });
    }
    steps
}

fn deferred_on_claude_max(
    tag: &str,
    tasks: &[crate::engine::topology::scaffold::WideTask],
    limit: &'static str,
) -> crate::engine::topology::scaffold::Wide {
    use crate::engine::topology::scaffold::ScaffoldAdapters;

    let mut wide = on_claude_max(tag, tasks, &[(TaskKey(0), GenerationId(0), limit)], &[]);
    let mut hooks = wide.env.hooks();
    let settled = step_answered_by(&mut wide, ScaffoldAdapters::rate_limiting(), &mut hooks);
    assert!(
        matches!(
            settled,
            Progress::Settled {
                key: TaskKey(0),
                accepted: false,
                ..
            }
        ),
        "{tag}: {settled:?}"
    );
    assert_eq!(
        wide.run.fold().task(TaskKey(0)).map(|task| task.state),
        Some(crate::topology::fold::TaskState::Deferred),
        "{tag}: the rate limit defers the task"
    );
    wide
}

#[test]
fn a_deferred_rate_limit_naming_a_reset_is_waited_out_to_two_minutes_past_it() {
    use crate::engine::topology::scaffold::WideTask;

    let mut wide = deferred_on_claude_max(
        "reset-wait-five-hour",
        &[WideTask::independent("alpha")],
        FIVE_HOUR_LIMIT,
    );
    assert_eq!(pools_exhausted(&wide), [signal("2026-11-11T05:30:00Z")]);
    let mut hooks = wide.env.hooks();

    let sleeper = RecordingSleeper::default();
    let steps = stepped_until_waited(&mut wide, &RunClock, &sleeper, None, &mut hooks);

    let slept = sleeper.slept();
    assert!(
        slept
            .iter()
            .all(|slice| *slice <= std::time::Duration::from_secs(600)),
        "every slice is at most ten minutes: {slept:?}"
    );
    assert_eq!(
        slept.iter().sum::<std::time::Duration>(),
        std::time::Duration::from_secs(9_120),
        "the run sleeps from 03:00:00 to 05:32:00, two minutes past the 05:30 reset: {slept:?}"
    );
    let (last, before) = steps.split_last().expect("at least one step");
    assert_eq!(
        last.defer_waits,
        [(9_120_000, 1)],
        "exactly one `defer_wait_elapsed`, for the whole wait, in the first round: {last:?}"
    );
    for step in before {
        assert!(
            step.defer_waits.is_empty(),
            "no `defer_wait_elapsed` before the last slice: {step:?}"
        );
    }
}

fn assert_backs_off_as_before(tag: &str, steps: &[Stepped], sleeper: &RecordingSleeper) {
    assert_eq!(
        sleeper.slept(),
        [std::time::Duration::from_secs(60)],
        "{tag}: one minute, the first round of the doubling backoff"
    );
    let [step] = steps else {
        panic!("{tag}: one step, which waited: {steps:?}");
    };
    assert_eq!(
        step.progress,
        Progress::Waited {
            waited_ms: 60_000,
            round: 1
        },
        "{tag}"
    );
    assert_eq!(step.defer_waits, [(60_000, 1)], "{tag}");
}

#[test]
fn a_limit_naming_a_reset_past_the_bound_backs_off_as_before() {
    use crate::engine::topology::scaffold::WideTask;

    let mut wide = deferred_on_claude_max(
        "reset-wait-weekly",
        &[WideTask::independent("alpha")],
        WEEKLY_LIMIT,
    );
    assert_eq!(
        pools_exhausted(&wide),
        [signal("2026-11-14T17:00:00Z")],
        "the weekly reset is three and a half days past the run's clock"
    );
    let mut hooks = wide.env.hooks();
    let sleeper = RecordingSleeper::default();
    let steps = stepped_until_waited(&mut wide, &RunClock, &sleeper, None, &mut hooks);
    assert_backs_off_as_before("weekly", &steps, &sleeper);
}

const UNREAD_LIMIT: &str = "429 Too Many Requests";

#[test]
fn a_rate_limit_whose_reset_is_not_read_backs_off_as_before() {
    use crate::engine::topology::scaffold::WideTask;

    let mut wide = deferred_on_claude_max(
        "reset-wait-unread",
        &[WideTask::independent("alpha")],
        UNREAD_LIMIT,
    );
    assert_eq!(
        pools_exhausted(&wide),
        [("claude-max".to_owned(), None)],
        "the pool is recorded exhausted, and no reset with it"
    );
    let mut hooks = wide.env.hooks();
    let sleeper = RecordingSleeper::default();
    let steps = stepped_until_waited(&mut wide, &RunClock, &sleeper, None, &mut hooks);
    assert_backs_off_as_before("unread", &steps, &sleeper);
}

struct AfterTheReset;

impl TimeSource for AfterTheReset {
    fn now_rfc3339(&self) -> String {
        "2026-11-11T06:00:00Z".to_owned()
    }
}

#[test]
fn a_recorded_reset_already_passed_backs_off_as_before() {
    use crate::engine::topology::scaffold::WideTask;

    let mut wide = deferred_on_claude_max(
        "reset-wait-passed",
        &[WideTask::independent("alpha")],
        FIVE_HOUR_LIMIT,
    );
    assert_eq!(pools_exhausted(&wide), [signal("2026-11-11T05:30:00Z")]);
    let mut hooks = wide.env.hooks();
    let sleeper = RecordingSleeper::default();
    let steps = stepped_until_waited(&mut wide, &AfterTheReset, &sleeper, None, &mut hooks);
    assert_backs_off_as_before("passed", &steps, &sleeper);
}

#[test]
fn a_reset_on_a_day_its_month_lacks_is_not_read() {
    for reading in [
        "2026-02-31T01:00:00Z",
        "2026-02-29T01:00:00Z",
        "2026-04-31T00:00:00Z",
    ] {
        assert_eq!(
            unix_seconds_of(reading),
            None,
            "{reading} names a day its month lacks, so it is no instant at all"
        );
    }
    let leap_day = u64::try_from(crate::util::days_from_civil(2028, 2, 29) * 86_400 + 3_600)
        .expect("29 February 2028 is after the epoch");
    assert_eq!(
        unix_seconds_of("2028-02-29T01:00:00Z"),
        Some(leap_day),
        "2028 is a leap year, so its 29 February is read"
    );
    assert_eq!(
        unix_seconds_of("2026-11-11T05:30:00Z"),
        Some(1_794_375_000),
        "the five-hour limit's reset reads as it always did"
    );
}

struct OnMarchThird;

impl TimeSource for OnMarchThird {
    fn now_rfc3339(&self) -> String {
        "2026-03-03T00:00:00Z".to_owned()
    }
}

#[test]
fn a_recorded_reset_on_a_day_its_month_lacks_backs_off_as_before() {
    use crate::engine::topology::scaffold::WideTask;

    let mut wide = deferred_on_claude_max(
        "reset-wait-no-such-day",
        &[WideTask::independent("alpha")],
        FIVE_HOUR_LIMIT,
    );
    wide.run.exhausted_pools = [signal("2026-02-31T01:00:00Z")].into_iter().collect();
    let mut hooks = wide.env.hooks();
    let sleeper = RecordingSleeper::default();
    let steps = stepped_until_waited(&mut wide, &OnMarchThird, &sleeper, None, &mut hooks);
    assert_backs_off_as_before("no such day", &steps, &sleeper);
}

#[test]
fn a_reset_reading_outside_the_writers_shape_is_not_read() {
    for reading in [
        "9223372036854775807-11-11T05:30:00Z",
        "10000-11-11T05:30:00Z",
        "2026-1-11T05:30:00Z",
        "2026-11-11T05:30:+0Z",
        "2026-11-11T05:30:00.not-a-fractionZ",
        "2026-11-11T05:30:00.Z",
        "2026-11-11t05:30:00Z",
        "2026-11-11T05:30:00z",
        "2026-11-11T05:30:00+00:00",
        "2026-11-11T05:30:60Z",
        "２０２６-11-11T05:30:00Z",
        "",
    ] {
        assert_eq!(
            unix_seconds_of(reading),
            None,
            "{reading:?} is not what `util::rfc3339_utc` writes, so it is not read"
        );
    }
    assert_eq!(
        unix_seconds_of("2026-11-11T05:30:00.123Z"),
        Some(1_794_375_000),
        "a fraction of digits is read, and its second is the whole second before it"
    );
    assert_eq!(
        unix_seconds_of("1970-01-01T00:00:00Z"),
        Some(0),
        "the epoch reads as zero"
    );
    assert_eq!(
        unix_seconds_of("9999-12-31T23:59:59Z"),
        Some(253_402_300_799),
        "the last second a four-digit year holds reads"
    );
}

#[test]
fn whatever_reads_is_what_the_writer_writes() {
    use crate::util::rfc3339_utc;

    const LAST: u64 = 253_402_300_799;
    let edges = [1, 59, 86_399, 951_782_400, 1_794_375_000, LAST];
    let spread = (0..=9_973_u64).map(|step| step * (LAST / 9_973));
    for instant in spread.chain(edges) {
        let written = rfc3339_utc(instant);
        assert_eq!(
            unix_seconds_of(&written),
            Some(instant),
            "{written} is what the writer writes for {instant}"
        );
    }

    let less_its_fraction = |text: &str| -> String {
        let Some((whole, rest)) = text.split_once('.') else {
            return text.to_owned();
        };
        match rest.strip_suffix('Z') {
            Some(fraction)
                if !fraction.is_empty() && fraction.bytes().all(|byte| byte.is_ascii_digit()) =>
            {
                format!("{whole}Z")
            }
            _ => text.to_owned(),
        }
    };
    let alphabet = ['0', '9', '+', '-', '.', ':', 'T', 'Z', 'é', '２', ' '];
    for reading in ["2026-11-11T05:30:00Z", "2028-02-29T23:59:59.5Z"] {
        let chars: Vec<char> = reading.chars().collect();
        let mut variants: Vec<String> = Vec::new();
        for at in 0..=chars.len() {
            let (before, from) = (chars.iter().take(at), chars.iter().skip(at));
            for with in alphabet {
                let inserted = before.clone().chain([&with]).chain(from.clone());
                variants.push(inserted.collect());
            }
        }
        for at in 0..chars.len() {
            let (before, after) = (chars.iter().take(at), chars.iter().skip(at + 1));
            variants.push(before.clone().chain(after.clone()).collect());
            for with in alphabet {
                let substituted = before.clone().chain([&with]).chain(after.clone());
                variants.push(substituted.collect());
            }
        }
        for variant in variants {
            if let Some(instant) = unix_seconds_of(&variant) {
                assert_eq!(
                    rfc3339_utc(instant),
                    less_its_fraction(&variant),
                    "{variant:?}, one character from {reading:?}, reads as {instant}, \
                     which the writer writes otherwise"
                );
            }
        }
    }
}

#[test]
fn a_recorded_reset_outside_the_writers_shape_backs_off_as_before() {
    use crate::engine::topology::scaffold::WideTask;

    for (tag, reset_at) in [
        (
            "reset-wait-oversized-year",
            "9223372036854775807-11-11T05:30:00Z",
        ),
        (
            "reset-wait-not-a-fraction",
            "2026-11-11T05:30:00.not-a-fractionZ",
        ),
    ] {
        let mut wide =
            deferred_on_claude_max(tag, &[WideTask::independent("alpha")], FIVE_HOUR_LIMIT);
        wide.run.exhausted_pools = [signal(reset_at)].into_iter().collect();
        let mut hooks = wide.env.hooks();
        let sleeper = RecordingSleeper::default();
        let steps = stepped_until_waited(&mut wide, &RunClock, &sleeper, None, &mut hooks);
        assert_backs_off_as_before(tag, &steps, &sleeper);
    }
}

struct AnsweredAfterSlices<'a> {
    sleeper: &'a RecordingSleeper,
    slices: usize,
    answer: Answer,
}

impl crate::interaction::AnswerSource for AnsweredAfterSlices<'_> {
    fn id(&self) -> &'static str {
        "after-slices"
    }

    fn resolve(&self, question: &Question) -> Result<Answer, UpstrokeError> {
        self.poll(question)
    }

    fn poll(&self, _question: &Question) -> Result<Answer, UpstrokeError> {
        Ok(if self.sleeper.slept().len() >= self.slices {
            self.answer.clone()
        } else {
            Answer::Unanswered
        })
    }
}

struct WaitingLines(std::sync::Mutex<Vec<String>>);

impl WaitingLines {
    fn lines(&self) -> Vec<String> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl Notifier for WaitingLines {
    fn id(&self) -> &'static str {
        "waiting-lines"
    }

    fn ask(&self, _question: &Question) -> Result<(), UpstrokeError> {
        Ok(())
    }

    fn waiting(&self, line: &str) {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(line.to_owned());
    }
}

static WAITING_LINES: WaitingLines = WaitingLines(std::sync::Mutex::new(Vec::new()));

#[test]
fn an_answer_published_between_two_slices_is_ingested_before_the_wait_continues() {
    use crate::engine::topology::scaffold::WideTask;

    let mut wide = deferred_on_claude_max(
        "reset-wait-answered",
        &[
            WideTask::independent("alpha"),
            WideTask::independent("beta"),
        ],
        FIVE_HOUR_LIMIT,
    );
    let mut hooks = wide.env.hooks();
    wide.run.approve_spend_over = Some(0.0);
    let asked = {
        let seams = RunSeams {
            clock: &RunClock,
            ..wide.env.seams()
        };
        wide.run
            .ask_to_approve_spend(TaskKey(1), &seams, &mut hooks)
            .expect("beta's question is raised")
    };
    assert!(
        matches!(
            asked,
            Progress::Asked {
                key: TaskKey(1),
                ..
            }
        ),
        "{asked:?}"
    );
    wide.run.notifiers = vec![&WAITING_LINES];

    let sleeper = RecordingSleeper::default();
    let answers = AnsweredAfterSlices {
        sleeper: &sleeper,
        slices: 2,
        answer: Answer::Declined,
    };
    let steps = stepped_until_waited(&mut wide, &RunClock, &sleeper, Some(&answers), &mut hooks);

    let answered = steps
        .iter()
        .position(|step| matches!(step.progress, Progress::Answered { .. }))
        .unwrap_or_else(|| panic!("the answer was ingested: {steps:?}"));
    assert_eq!(
        answered, 2,
        "two slices, then the answer published after the second: {steps:?}"
    );
    assert_eq!(
        steps[answered].progress,
        Progress::Answered {
            key: TaskKey(1),
            question: match &asked {
                Progress::Asked { question, .. } => question.clone(),
                other => panic!("{other:?}"),
            },
            declined: true,
        }
    );
    assert_eq!(
        steps[answered].slices, 2,
        "the step that ingested the answer slept no slice of its own"
    );
    for (at, step) in steps.iter().enumerate() {
        if at != answered && at + 1 != steps.len() {
            assert!(
                matches!(step.progress, Progress::Waiting { until_unix, slept_ms }
                    if until_unix == 1_794_375_120 && slept_ms <= 600_000),
                "every other step but the last sleeps one slice toward 05:32: {step:?}"
            );
        }
        assert!(
            at + 1 == steps.len() || step.defer_waits.is_empty(),
            "no `defer_wait_elapsed` before the last slice: {step:?}"
        );
    }
    assert_eq!(
        sleeper.slept().iter().sum::<std::time::Duration>(),
        std::time::Duration::from_secs(9_120),
        "the wait continued after the answer rather than starting again: {:?}",
        sleeper.slept()
    );
    assert_eq!(
        steps.last().map(|step| step.defer_waits.clone()),
        Some(vec![(9_120_000, 1)]),
        "one `defer_wait_elapsed`, for the whole wait"
    );
    assert_eq!(
        WAITING_LINES.lines(),
        [crate::interaction::reset_wait_line(
            "claude-max",
            "2026-11-11T05:30:00Z",
            std::time::Duration::from_secs(9_120),
        )],
        "the wait is announced once, when it starts, and not again when it continues"
    );
}

struct MovingClock<'a>(&'a RecordingSleeper);

impl TimeSource for MovingClock<'_> {
    fn now_rfc3339(&self) -> String {
        let from = unix_seconds_of(&RunClock.now_rfc3339()).expect("`RunClock` reads as UTC");
        let slept: std::time::Duration = self.0.slept().iter().sum();
        crate::util::rfc3339_utc(from + slept.as_secs())
    }
}

#[test]
fn a_held_reset_wait_runs_through_its_margin_on_a_moving_clock() {
    use crate::engine::topology::scaffold::WideTask;

    let mut wide = deferred_on_claude_max(
        "reset-wait-moving-clock",
        &[WideTask::independent("alpha")],
        FIVE_HOUR_LIMIT,
    );
    assert_eq!(pools_exhausted(&wide), [signal("2026-11-11T05:30:00Z")]);
    let mut hooks = wide.env.hooks();

    let sleeper = RecordingSleeper::default();
    let clock = MovingClock(&sleeper);
    let steps = stepped_until_waited(&mut wide, &clock, &sleeper, None, &mut hooks);

    let slept = sleeper.slept();
    assert_eq!(
        slept.iter().sum::<std::time::Duration>(),
        std::time::Duration::from_secs(9_120),
        "the run sleeps from 03:00:00 to 05:32:00, through the margin past the 05:30 reset: \
         {slept:?}"
    );
    assert!(
        slept
            .iter()
            .all(|slice| *slice <= std::time::Duration::from_secs(600)),
        "every slice is at most ten minutes: {slept:?}"
    );
    assert_eq!(
        clock.now_rfc3339(),
        "2026-11-11T05:32:00Z",
        "the task wakes two minutes past the reset, not before"
    );
    assert_eq!(
        steps.last().map(|step| step.defer_waits.clone()),
        Some(vec![(9_120_000, 1)]),
        "exactly one `defer_wait_elapsed`, for the whole wait, in the first round: {steps:?}"
    );
}

#[test]
fn every_elapsed_wait_records_all_of_its_sleeps() {
    use crate::engine::topology::scaffold::WideTask;

    let mut wide = deferred_on_claude_max(
        "reset-wait-every-sleep",
        &[WideTask::independent("alpha")],
        FIVE_HOUR_LIMIT,
    );
    let mut hooks = wide.env.hooks();
    let mut recorded = defer_waits(&wide).len();

    let sleeper = RecordingSleeper::default();
    let steps = stepped_until_waited(
        &mut wide,
        &MovingClock(&sleeper),
        &sleeper,
        None,
        &mut hooks,
    );

    let slept = sleeper.slept();
    let mut since = 0;
    let mut checked = 0;
    for step in &steps {
        for (waited_ms, round) in &step.defer_waits[recorded..] {
            let sleeps = &slept[since..step.slices];
            assert_eq!(
                std::time::Duration::from_millis(*waited_ms),
                sleeps.iter().sum::<std::time::Duration>(),
                "the `defer_wait_elapsed` of round {round} records every sleep since the one \
                 before it: {sleeps:?}"
            );
            since = step.slices;
            checked += 1;
        }
        recorded = step.defer_waits.len();
    }
    assert!(
        checked > 0,
        "the step that answered `Waited` appended a `defer_wait_elapsed`: {steps:?}"
    );
}

#[test]
fn the_serving_pool_is_the_first_in_file_order_no_signal_holds_exhausted() {
    const AHEAD: Option<&str> = Some("2026-11-11T05:30:00Z");
    const PASSED_BY_THE_MARGIN: Option<&str> = Some("2026-11-11T02:58:00Z");
    const PASSED_SHORT_OF_THE_MARGIN: Option<&str> = Some("2026-11-11T02:58:01Z");
    const ON_NO_SUCH_DAY: Option<&str> = Some("2026-02-31T01:00:00Z");
    type Held = [(&'static str, Option<&'static str>)];

    let candidates = ["a".to_owned(), "b".to_owned(), "c".to_owned()];
    let now = unix_seconds_of(&RunClock.now_rfc3339()).expect("`RunClock` reads as UTC");
    assert_eq!(
        PASSED_BY_THE_MARGIN
            .and_then(unix_seconds_of)
            .map(|reset| reset + crate::interaction::RESET_WAIT_MARGIN.as_secs()),
        Some(now),
        "the passed reset is `RESET_WAIT_MARGIN` behind the clock to the second, so it is the \
         boundary the rule's `at or before` is about"
    );

    let cases: [(&str, &Held, &str); 9] = [
        ("nothing is held", &[], "a"),
        ("`a` is held", &[("a", AHEAD)], "b"),
        ("`a` and `b` are held", &[("a", AHEAD), ("b", None)], "c"),
        (
            "a pool that is not a candidate is held",
            &[("claude-max", AHEAD)],
            "a",
        ),
        (
            "all three are held, and `b`'s reset passed by the margin",
            &[("a", AHEAD), ("b", PASSED_BY_THE_MARGIN), ("c", AHEAD)],
            "b",
        ),
        (
            "all three are held, and `b`'s reset passed one second short of the margin",
            &[
                ("a", AHEAD),
                ("b", PASSED_SHORT_OF_THE_MARGIN),
                ("c", AHEAD),
            ],
            "a",
        ),
        (
            "all three are held, and no reset has passed",
            &[("a", AHEAD), ("b", AHEAD), ("c", AHEAD)],
            "a",
        ),
        (
            "all three are held, and no reset is known",
            &[("a", None), ("b", None), ("c", None)],
            "a",
        ),
        (
            "all three are held, and `b`'s reset is on a day its month lacks",
            &[("a", AHEAD), ("b", ON_NO_SUCH_DAY), ("c", AHEAD)],
            "a",
        ),
    ];
    for (case, held, expected) in cases {
        let held: BTreeMap<String, Option<String>> = held
            .iter()
            .map(|(pool, reset)| ((*pool).to_owned(), reset.map(str::to_owned)))
            .collect();
        assert_eq!(
            serving_pool(&candidates, &held, now).map(String::as_str),
            Some(expected),
            "{case}: {held:?}"
        );
    }

    assert_eq!(
        serving_pool(&[], &BTreeMap::new(), now),
        None,
        "an agent with no pool binds none"
    );
}

fn on_two_claude_max_pools(
    tag: &str,
    tasks: &[crate::engine::topology::scaffold::WideTask],
    limited: &[(TaskKey, GenerationId, &'static str)],
    failing_gates: &[(u32, u32)],
) -> crate::engine::topology::scaffold::Wide {
    use crate::engine::topology::scaffold::{AGENT, Wide, WidePlans};

    Wide::started_with(
        tag,
        tasks,
        1,
        WidePlans {
            pools: vec![
                (AGENT, Some("claude-max".to_owned())),
                (AGENT, Some("claude-max-2".to_owned())),
            ],
            ..WidePlans::default()
        },
        limited_runner(tasks, limited, failing_gates),
    )
}

fn served_steps_until_settled(
    wide: &mut crate::engine::topology::scaffold::Wide,
    key: TaskKey,
    hooks: &mut dyn TopologyHooks,
) -> Vec<Progress> {
    use crate::engine::topology::scaffold::ScaffoldAdapters;

    let mut steps = Vec::new();
    while steps.len() < 64 {
        let progress = step_answered_by(wide, ScaffoldAdapters::new(), hooks);
        let settled = matches!(&progress, Progress::Settled { key: of, .. } if *of == key);
        steps.push(progress);
        if settled {
            return steps;
        }
    }
    panic!("task {key} settled no attempt in 64 steps: {steps:?}");
}

fn attempts_started(
    wide: &crate::engine::topology::scaffold::Wide,
    key: TaskKey,
) -> Vec<crate::topology::events::AttemptStarted4> {
    wide.env
        .durable_events()
        .into_iter()
        .filter_map(|event| match event.body {
            TopologyEventBody::AttemptStarted { data } if data.key == key => Some(data),
            _ => None,
        })
        .collect()
}

fn limited_once_on_two_pools(
    tag: &str,
    failing_gates: &[(u32, u32)],
) -> crate::engine::topology::scaffold::Wide {
    use crate::engine::topology::scaffold::{ScaffoldAdapters, WideTask};

    let mut wide = on_two_claude_max_pools(
        tag,
        &[WideTask::independent("alpha")],
        &[(TaskKey(0), GenerationId(0), FIVE_HOUR_LIMIT)],
        failing_gates,
    );
    let mut hooks = wide.env.hooks();
    let limited = step_answered_by(&mut wide, ScaffoldAdapters::rate_limiting(), &mut hooks);
    assert!(
        matches!(
            limited,
            Progress::Settled {
                key: TaskKey(0),
                accepted: false,
                ..
            }
        ),
        "{tag}: generation 0 is rate limited: {limited:?}"
    );
    assert_eq!(
        pools_exhausted(&wide),
        [signal("2026-11-11T05:30:00Z")],
        "{tag}: the limit is signalled on `claude-max`, the first pool of the agent"
    );
    wide
}

#[test]
fn a_task_dispatched_again_after_a_rate_limit_binds_the_next_pool_of_its_agent_in_a_fresh_session()
{
    let mut wide = limited_once_on_two_pools("serving-pool-next-of-agent", &[]);
    let mut hooks = wide.env.hooks();
    let steps = served_steps_until_settled(&mut wide, TaskKey(0), &mut hooks);
    assert!(
        matches!(
            steps.last(),
            Some(Progress::Settled {
                key: TaskKey(0),
                accepted: true,
                ..
            })
        ),
        "the task's next generation is served and accepted, after any wait: {steps:?}"
    );

    let started = attempts_started(&wide, TaskKey(0));
    assert_eq!(
        started
            .iter()
            .map(|attempt| (
                attempt.generation,
                attempt.attempt,
                attempt.pool.as_deref(),
                attempt.resume_session.as_ref(),
            ))
            .collect::<Vec<_>>(),
        [
            (GenerationId(0), AttemptNumber(1), Some("claude-max"), None),
            (
                GenerationId(1),
                AttemptNumber(1),
                Some("claude-max-2"),
                None
            ),
        ],
        "generation 0 on the agent's first pool, and generation 1 on its next one while a signal \
         holds the first exhausted, each in a fresh session: {steps:?}"
    );
    let [first, second] = started.as_slice() else {
        panic!("exactly two attempts started: {started:?}");
    };
    assert_eq!(
        first.binding, second.binding,
        "the pool moved and the binding did not: the same agent, model and effort on both"
    );
}

#[test]
fn a_retained_retry_resumes_on_the_pool_its_session_ran_on() {
    let mut wide = limited_once_on_two_pools("serving-pool-retained-retry", &[(0, 1)]);
    let mut hooks = wide.env.hooks();
    let steps = served_steps_until_settled(&mut wide, TaskKey(0), &mut hooks);
    assert!(
        matches!(
            steps.last(),
            Some(Progress::Settled {
                key: TaskKey(0),
                accepted: false,
                ..
            })
        ),
        "generation 1's first attempt fails its gate: {steps:?}"
    );
    assert_eq!(
        failures_of(&wide, TaskKey(0)),
        [
            Some(crate::ladder::FailureKind::RateLimited),
            Some(crate::ladder::FailureKind::GateFailed)
        ],
        "{steps:?}"
    );

    wide.run.exhausted_pools.remove("claude-max");
    let retried = served_steps_until_settled(&mut wide, TaskKey(0), &mut hooks);
    assert!(
        matches!(
            retried.as_slice(),
            [Progress::Settled {
                key: TaskKey(0),
                accepted: true,
                ..
            }]
        ),
        "the retained attempt is retried at once, and its second attempt is accepted: {retried:?}"
    );

    let started = attempts_started(&wide, TaskKey(0));
    let retry = started.last().expect("the retry started");
    assert_eq!(
        (retry.generation, retry.attempt),
        (GenerationId(1), AttemptNumber(2)),
        "the last attempt started is the retry in the retained generation: {started:?}"
    );
    assert!(
        retry.resume_session.is_some(),
        "the retry resumes the retained attempt's session: {retry:?}"
    );
    assert_eq!(
        retry.pool.as_deref(),
        Some("claude-max-2"),
        "the retry resumes on the pool its session ran on, though `claude-max` is first in file \
         order and no signal holds it exhausted any longer: {started:?}"
    );
    assert_eq!(
        started
            .iter()
            .map(|attempt| attempt.pool.as_deref())
            .collect::<Vec<_>>(),
        [
            Some("claude-max"),
            Some("claude-max-2"),
            Some("claude-max-2")
        ],
        "generation 0 on the first pool, then both attempts of generation 1 on the next"
    );
}

fn last_settlement(
    wide: &crate::engine::topology::scaffold::Wide,
    key: TaskKey,
) -> Option<AttemptSettlement> {
    wide.env
        .durable_events()
        .into_iter()
        .rev()
        .find_map(|event| match event.body {
            TopologyEventBody::AttemptFinished { data } if data.key == key => Some(data.settlement),
            _ => None,
        })
}

fn deferral_of(wide: &crate::engine::topology::scaffold::Wide, key: TaskKey) -> (TaskState, u32) {
    let task = wide.run.fold().task(key).expect("the task is in the fold");
    (task.state, task.defers)
}

#[test]
fn a_rate_limit_fails_over_to_the_next_pool_of_the_same_agent_in_a_fresh_session() {
    use crate::engine::topology::scaffold::{ScaffoldAdapters, WideTask};

    let mut wide = on_two_claude_max_pools(
        "failover-next-pool",
        &[WideTask::independent("alpha")],
        &[(TaskKey(0), GenerationId(0), FIVE_HOUR_LIMIT)],
        &[],
    );
    let mut hooks = wide.env.hooks();
    let limited = step_answered_by(&mut wide, ScaffoldAdapters::rate_limiting(), &mut hooks);
    assert_eq!(
        limited,
        Progress::Settled {
            key: TaskKey(0),
            accepted: false,
            spent_attempt: false,
        },
        "generation 0 is rate limited, and an outage spends none of the rung's allowance"
    );
    let (state, defers) = deferral_of(&wide, TaskKey(0));
    assert_ne!(
        state,
        TaskState::Deferred,
        "`claude-max-2` is not held exhausted, so the limit on `claude-max` defers nothing"
    );
    assert_eq!(
        defers, 0,
        "a failover spends none of the task's `max_defers`"
    );
    let settlement = last_settlement(&wide, TaskKey(0));
    assert!(
        matches!(
            settlement,
            Some(AttemptSettlement::Closed {
                transition: SettlementTransition::Retry,
                ..
            })
        ),
        "the generation closes for a retry in a fresh one: {settlement:?}"
    );
    assert_eq!(
        pools_exhausted(&wide),
        [signal("2026-11-11T05:30:00Z")],
        "the pool that refused is still signalled exhausted, with its reset"
    );

    wide.env.adapters = std::sync::Arc::new(ScaffoldAdapters::new());
    let sleeper = RecordingSleeper::default();
    let served = {
        let seams = RunSeams {
            clock: &RunClock,
            sleeper: &sleeper,
            ..wide.env.seams()
        };
        wide.run.step(&seams, &mut hooks).expect("a step")
    };
    assert!(
        matches!(
            served,
            Progress::Settled {
                key: TaskKey(0),
                accepted: true,
                ..
            }
        ),
        "the very next step runs generation 1, which is served and accepted: {served:?}"
    );
    assert_eq!(sleeper.slept(), [], "nothing was slept between the two");
    assert_eq!(defer_waits(&wide), [], "and no wait was recorded");

    let started = attempts_started(&wide, TaskKey(0));
    let [first, second] = started.as_slice() else {
        panic!("exactly two attempts started: {started:?}");
    };
    assert_eq!(
        (
            second.generation,
            second.attempt,
            second.pool.as_deref(),
            second.resume_session.as_ref(),
        ),
        (
            GenerationId(1),
            AttemptNumber(1),
            Some("claude-max-2"),
            None
        ),
        "generation 1 starts on the agent's next pool, in a fresh session: {started:?}"
    );
    assert_eq!(
        first.binding, second.binding,
        "the pool moved and the binding did not: the same agent, model and effort on both"
    );
}

const LIMIT_RESET_A_MINUTE_AGO: &str = "You've hit your 5-hour limit · resets 2:59am (UTC)";

#[test]
fn a_failover_never_returns_to_the_pool_that_refused_even_when_its_reset_reads_as_past() {
    use crate::engine::topology::scaffold::{ScaffoldAdapters, WideTask};

    let mut wide = on_two_claude_max_pools(
        "failover-reset-reads-as-past",
        &[WideTask::independent("alpha")],
        &[(TaskKey(0), GenerationId(0), LIMIT_RESET_A_MINUTE_AGO)],
        &[],
    );
    let mut hooks = wide.env.hooks();
    let limited = step_answered_by(&mut wide, ScaffoldAdapters::rate_limiting(), &mut hooks);
    assert!(
        matches!(
            limited,
            Progress::Settled {
                key: TaskKey(0),
                accepted: false,
                ..
            }
        ),
        "generation 0 is rate limited: {limited:?}"
    );
    assert_eq!(
        pools_exhausted(&wide),
        [signal("2026-11-11T02:59:00Z")],
        "this test needs the limit read as the 02:59 a minute behind the 03:00 clock, and it was \
         read as another instant, so nothing below it says anything"
    );
    assert_ne!(
        deferral_of(&wide, TaskKey(0)).0,
        TaskState::Deferred,
        "the limit fails over: `claude-max-2` is not held exhausted"
    );

    let served = step_answered_by(&mut wide, ScaffoldAdapters::new(), &mut hooks);
    let started = attempts_started(&wide, TaskKey(0));
    assert_eq!(
        started
            .iter()
            .map(|attempt| (attempt.generation, attempt.pool.as_deref()))
            .collect::<Vec<_>>(),
        [
            (GenerationId(0), Some("claude-max")),
            (GenerationId(1), Some("claude-max-2")),
        ],
        "generation 1 binds `claude-max-2`, and not the `claude-max` that had just refused \
         generation 0, though that pool's recorded reset is behind the clock: {served:?}"
    );
}

const SEVEN_AM_LIMIT: &str = "You've hit your 5-hour limit · resets 7am (UTC)";

#[test]
fn with_every_pool_of_the_agent_exhausted_the_run_waits_for_the_earliest_reset_among_them() {
    use crate::engine::topology::scaffold::{ScaffoldAdapters, WideTask};

    let mut wide = on_two_claude_max_pools(
        "failover-every-pool-exhausted",
        &[WideTask::independent("alpha")],
        &[
            (TaskKey(0), GenerationId(0), FIVE_HOUR_LIMIT),
            (TaskKey(0), GenerationId(1), SEVEN_AM_LIMIT),
        ],
        &[],
    );
    let mut hooks = wide.env.hooks();
    let limited: Vec<Progress> = (0..2)
        .map(|_| step_answered_by(&mut wide, ScaffoldAdapters::rate_limiting(), &mut hooks))
        .collect();
    assert!(
        matches!(
            limited.as_slice(),
            [
                Progress::Settled {
                    key: TaskKey(0),
                    accepted: false,
                    ..
                },
                Progress::Settled {
                    key: TaskKey(0),
                    accepted: false,
                    ..
                },
            ]
        ),
        "generation 0 is limited on `claude-max` and fails over, and generation 1 is limited on \
         `claude-max-2` in the very next step: {limited:?}"
    );
    assert_eq!(
        pools_exhausted(&wide),
        [
            signal("2026-11-11T05:30:00Z"),
            (
                "claude-max-2".to_owned(),
                Some("2026-11-11T07:00:00Z".to_owned())
            ),
        ],
        "each pool is signalled exhausted with the reset its own limit named"
    );
    assert_eq!(
        deferral_of(&wide, TaskKey(0)),
        (TaskState::Deferred, 1),
        "with both of the agent's pools held exhausted, the second limit defers the task, once"
    );

    let lines: &'static WaitingLines =
        Box::leak(Box::new(WaitingLines(std::sync::Mutex::new(Vec::new()))));
    wide.run.notifiers = vec![lines];
    let sleeper = RecordingSleeper::default();
    let steps = stepped_until_waited(&mut wide, &RunClock, &sleeper, None, &mut hooks);

    let slept = sleeper.slept();
    assert!(
        slept
            .iter()
            .all(|slice| *slice <= std::time::Duration::from_secs(600)),
        "every slice is at most ten minutes: {slept:?}"
    );
    assert_eq!(
        slept.iter().sum::<std::time::Duration>(),
        std::time::Duration::from_secs(9_120),
        "the run sleeps from 03:00:00 to 05:32:00, two minutes past `claude-max`'s 05:30 reset, \
         and not to 07:02:00, two minutes past the reset of the `claude-max-2` that refused \
         last: {slept:?}"
    );
    assert_eq!(
        steps.last().map(|step| step.defer_waits.clone()),
        Some(vec![(9_120_000, 1)]),
        "exactly one `defer_wait_elapsed`, for the whole wait, in the first round: {steps:?}"
    );
    assert_eq!(
        lines.lines(),
        [crate::interaction::reset_wait_line(
            "claude-max",
            "2026-11-11T05:30:00Z",
            std::time::Duration::from_secs(9_120),
        )],
        "the wait is announced for the pool whose reset it runs to"
    );
}

#[test]
fn a_rate_limit_with_no_other_pool_for_the_agent_still_defers() {
    use crate::engine::topology::scaffold::{
        AGENT, REVIEW_AGENT, ScaffoldAdapters, Wide, WidePlans, WideTask,
    };

    let tasks = [WideTask::independent("alpha")];
    let mut wide = Wide::started_with(
        "failover-no-other-pool",
        &tasks,
        1,
        WidePlans {
            pools: vec![
                (AGENT, Some("claude-max".to_owned())),
                (REVIEW_AGENT, Some("copilot-pool".to_owned())),
            ],
            ..WidePlans::default()
        },
        limited_runner(
            &tasks,
            &[(TaskKey(0), GenerationId(0), FIVE_HOUR_LIMIT)],
            &[],
        ),
    );
    let mut hooks = wide.env.hooks();
    let limited = step_answered_by(&mut wide, ScaffoldAdapters::rate_limiting(), &mut hooks);
    assert_eq!(
        limited,
        Progress::Settled {
            key: TaskKey(0),
            accepted: false,
            spent_attempt: false,
        }
    );
    assert_eq!(
        deferral_of(&wide, TaskKey(0)),
        (TaskState::Deferred, 1),
        "`copilot-pool` is another agent's, so there is no pool to fail over to and the limit \
         defers the task"
    );
    let settlement = last_settlement(&wide, TaskKey(0));
    assert!(
        matches!(
            settlement,
            Some(AttemptSettlement::Closed {
                transition: SettlementTransition::Deferred { defers: 1, .. },
                ..
            })
        ),
        "{settlement:?}"
    );
    assert_eq!(pools_exhausted(&wide), [signal("2026-11-11T05:30:00Z")]);

    let sleeper = RecordingSleeper::default();
    let steps = stepped_until_waited(&mut wide, &RunClock, &sleeper, None, &mut hooks);
    assert_eq!(
        sleeper.slept().iter().sum::<std::time::Duration>(),
        std::time::Duration::from_secs(9_120),
        "the deferral waits to 05:32:00, two minutes past the reset of the agent's one pool"
    );
    assert_eq!(
        steps.last().map(|step| step.defer_waits.clone()),
        Some(vec![(9_120_000, 1)]),
        "{steps:?}"
    );
    assert_eq!(
        attempts_started(&wide, TaskKey(0))
            .iter()
            .map(|attempt| attempt.pool.as_deref())
            .collect::<Vec<_>>(),
        [Some("claude-max")],
        "nothing ran on another agent's pool"
    );
}

#[test]
fn a_reviewers_rate_limit_still_defers() {
    use crate::engine::topology::scaffold::{
        AGENT, REVIEW_AGENT, RecordingRunner, ScaffoldAdapters, Wide, WidePlans, WideTask,
        wide_responder,
    };

    let tasks = [WideTask::independent("alpha")];
    let mut wide = Wide::started_with(
        "failover-reviewer-limit",
        &tasks,
        1,
        WidePlans {
            reviewers: 2,
            pools: vec![
                (AGENT, Some("claude-max".to_owned())),
                (AGENT, Some("claude-max-2".to_owned())),
                (REVIEW_AGENT, Some("copilot-pool".to_owned())),
                (REVIEW_AGENT, Some("copilot-pool-2".to_owned())),
            ],
            ..WidePlans::default()
        },
        RecordingRunner::new().answering(wide_responder(&tasks, &[])),
    );
    let mut hooks = wide.env.hooks();
    let limited = step_answered_by(
        &mut wide,
        ScaffoldAdapters::rate_limiting_the_second_reviewer(),
        &mut hooks,
    );
    assert_eq!(
        limited,
        Progress::Settled {
            key: TaskKey(0),
            accepted: false,
            spent_attempt: false,
        }
    );
    let failure = wide
        .env
        .durable_events()
        .into_iter()
        .find_map(|event| match event.body {
            TopologyEventBody::AttemptFinished { data } => data.record.failure,
            _ => None,
        })
        .expect("the attempt settled with a failure");
    assert_eq!(
        (failure.kind, failure.origin),
        (
            crate::ladder::FailureKind::RateLimited,
            crate::ladder::FailureOrigin::Reviewer
        ),
        "the worker was served, and its second reviewer was rate limited"
    );
    assert_eq!(
        pools_exhausted(&wide),
        [("copilot-pool".to_owned(), None)],
        "the reviewer's pool is signalled, with no reset: only Claude Code's is read"
    );
    assert_eq!(
        deferral_of(&wide, TaskKey(0)),
        (TaskState::Deferred, 1),
        "a reviewer's limit defers the task, though neither `claude-max-2` nor `copilot-pool-2` \
         is held exhausted: only a worker's limit fails over"
    );

    let sleeper = RecordingSleeper::default();
    let steps = stepped_until_waited(&mut wide, &RunClock, &sleeper, None, &mut hooks);
    assert_backs_off_as_before("reviewer", &steps, &sleeper);
}

fn escalated_log(interrupted_first: bool) -> (TopologyFold, Vec<TopologyEvent>) {
    use crate::engine::topology::settle::tests::{
        ALEPH, apply, attempt_started, dispatch, ev, finished, settle_into, started,
    };
    use crate::topology::events::{AttemptInterrupted4, LeaseDisposition};

    let mut fold = started();
    let mut events = Vec::new();
    let mut generation = 0;
    if interrupted_first {
        for event in [
            dispatch(ALEPH, 0),
            attempt_started(&fold, ALEPH, 0, 1),
            ev(TopologyEventBody::AttemptInterrupted {
                data: AttemptInterrupted4 {
                    key: ALEPH,
                    generation: GenerationId(0),
                    attempt: AttemptNumber(1),
                    lease: LeaseDisposition::PredictedReleased,
                    detail: "the coordinator that started this attempt did not survive it"
                        .to_owned(),
                },
            }),
        ] {
            apply(&mut fold, &event);
            events.push(event);
        }
        generation = 1;
    }
    let dispatched = dispatch(ALEPH, generation);
    apply(&mut fold, &dispatched);
    events.push(dispatched);
    let start = attempt_started(&fold, ALEPH, generation, 1);
    apply(&mut fold, &start);
    events.push(start);
    let settled = settle_into(
        &mut fold,
        &finished(ALEPH, generation, 1, crate::ladder::Next::Escalate),
    );
    events.push(ev(TopologyEventBody::AttemptFinished {
        data: Box::new(settled),
    }));
    (fold, events)
}

#[test]
fn a_spend_approval_after_an_interrupted_attempt_calls_its_reported_spend_a_floor_as_the_report_does()
 {
    use crate::engine::topology::settle::tests::ALEPH;

    for (interrupted_first, floor) in [(true, true), (false, false)] {
        let (fold, events) = escalated_log(interrupted_first);
        let spent = Spend::replay(&events).run_usd();
        let context = spend_approval_context(&fold, &events, ALEPH, spent, 0.005)
            .expect("the approval question's context");
        assert!(
            context.contains("Reported spend so far: $0.5000"),
            "{interrupted_first}: the priced escalation's spend is reported: {context}"
        );
        assert_eq!(
            context.contains("a floor, not a total")
                && context.contains("This is at least what the run has already cost"),
            floor,
            "interrupted first: {interrupted_first}: {context}"
        );
        assert_eq!(
            context.contains("This is what the run has already cost"),
            !floor,
            "interrupted first: {interrupted_first}: {context}"
        );
        assert_eq!(
            crate::engine::RunReport::from_topology(&fold, &events, Vec::new()).total_is_floor(),
            floor,
            "interrupted first: {interrupted_first}: the report and the approval treat the \
             run's unknown spend alike"
        );
    }
}
