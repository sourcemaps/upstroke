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
            "the retry arm: `request.pool`, which the driver fills from `AttemptPlans::pool_for` \
             — the same authority, asked one step earlier",
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
