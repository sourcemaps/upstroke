//! Extended notes: `docs/internals/answer.md`

// LEGACY-EFFECT: this module is in the frozen legacy section of
// `effects/allowlist.toml`, which carries its justification.
#![allow(clippy::disallowed_methods)]

use std::path::Path;

use crate::engine::topology::observe::{self, FoundOpen};
use crate::error::UpstrokeError;
use crate::interaction::{self, QuestionRecord};
use crate::ir::{Answer, Question, QuestionId};
use crate::rundir::{self, FoundQuestion, RunDirClass};

#[derive(Debug, Clone)]
pub enum Reply {
    Option(usize),
    Text(String),
    Decline,
}

#[derive(Debug)]
pub struct Answered {
    pub run_id: String,
    pub question_id: String,
    pub answer: Answer,
    pub run_is_live: bool,
}

pub fn answer(repo_root: &Path, wanted: &str, reply: Reply) -> Result<Answered, UpstrokeError> {
    let found = match locate(repo_root, wanted)? {
        Located::Legacy(found) => found,
        Located::Topology(open) => return answer_open(open, reply),
    };
    let id = QuestionId(found.question_id.clone());
    let questions = found.public.join("questions");

    let path = questions.join(format!("{}.json", found.question_id));
    let text = std::fs::read_to_string(&path).map_err(|source| UpstrokeError::Io {
        path: path.clone(),
        source,
    })?;
    let record: QuestionRecord = serde_json::from_str(&text).map_err(|e| UpstrokeError::Parse {
        message: format!("{}: {e}", path.display()),
    })?;

    if let Some(existing) = &record.answer {
        return Err(UpstrokeError::Refused {
            message: format!(
                "question {} was already answered ({}). Answers are applied once; if the task \
                 needs different guidance it will raise a new question.",
                found.question_id,
                describe(existing)
            ),
        });
    }

    let answer = reply_answer(&record.question, reply)?;

    let answers = found.public.join("answers");
    std::fs::create_dir_all(&answers).map_err(|source| UpstrokeError::Io {
        path: answers.clone(),
        source,
    })?;
    let record = interaction::AnswerRecord::unattributed(answer);
    interaction::write_answer(&answers, &id, &record)?;

    let run_is_live = rundir::is_running(&found.public);
    Ok(Answered {
        run_id: found.run_id,
        question_id: found.question_id,
        answer: record.answer,
        run_is_live,
    })
}

pub fn show(repo_root: &Path, wanted: &str) -> Result<String, UpstrokeError> {
    let found = match locate(repo_root, wanted)? {
        Located::Legacy(found) => found,
        Located::Topology(open) => return Ok(interaction::render_question(&open.asked.question)),
    };
    let path = found
        .public
        .join("questions")
        .join(format!("{}.json", found.question_id));
    let text = std::fs::read_to_string(&path).map_err(|source| UpstrokeError::Io {
        path: path.clone(),
        source,
    })?;
    let record: QuestionRecord = serde_json::from_str(&text).map_err(|e| UpstrokeError::Parse {
        message: format!("{}: {e}", path.display()),
    })?;
    Ok(interaction::render_question(&record.question))
}

fn reply_answer(question: &Question, reply: Reply) -> Result<Answer, UpstrokeError> {
    let answer = match reply {
        Reply::Decline => Answer::Declined,
        Reply::Text(text) => interaction::interpret(question, &text),
        Reply::Option(choice) => {
            interaction::answer_for_option(question, choice).ok_or_else(|| {
                UpstrokeError::Refused {
                    message: format!(
                        "there is no option {choice} on this question; it offers {}",
                        if question.options.is_empty() {
                            "none (answer it in your own words instead)".to_owned()
                        } else {
                            format!("1-{}", question.options.len())
                        }
                    ),
                }
            })?
        }
    };
    if answer == Answer::Unanswered {
        return Err(UpstrokeError::Refused {
            message: "an empty answer would leave the task exactly as it is; pass --text with \
                      the guidance, --option N, or --decline to give up on the task"
                .to_owned(),
        });
    }
    Ok(answer)
}

enum Located {
    Legacy(FoundQuestion),
    Topology(FoundOpen),
}

struct Census {
    committed: Vec<String>,
    unclassified: Vec<String>,
}

fn census(repo_root: &Path) -> Census {
    let mut census = Census {
        committed: Vec::new(),
        unclassified: Vec::new(),
    };
    for run_id in rundir::run_dir_names(repo_root) {
        match rundir::classify_run_dir(&rundir::public_dir(repo_root, &run_id)) {
            RunDirClass::Committed => census.committed.push(run_id),
            RunDirClass::Husk => {}
            RunDirClass::Indeterminate => census.unclassified.push(run_id),
        }
    }
    census
}

struct LegacyCandidates {
    exact: Option<FoundQuestion>,
    prefixed: Vec<FoundQuestion>,
}

fn legacy_candidates(repo_root: &Path, runs: &[String], wanted: &str) -> LegacyCandidates {
    let wanted_upper = wanted.to_ascii_uppercase();
    let mut candidates = LegacyCandidates {
        exact: None,
        prefixed: Vec::new(),
    };
    for run_id in runs {
        let public = rundir::public_dir(repo_root, run_id);
        let Ok(entries) = std::fs::read_dir(public.join("questions")) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(question_id) = name.strip_suffix(".json") else {
                continue;
            };
            let found = FoundQuestion {
                run_id: run_id.clone(),
                public: public.clone(),
                question_id: question_id.to_owned(),
            };
            if question_id.eq_ignore_ascii_case(wanted) {
                candidates.exact = Some(found);
            } else if question_id.to_ascii_uppercase().starts_with(&wanted_upper) {
                candidates.prefixed.push(found);
            }
        }
    }
    candidates
}

fn locate(repo_root: &Path, wanted: &str) -> Result<Located, UpstrokeError> {
    let census = census(repo_root);
    let topology = observe::open_question_candidates(repo_root, &census.committed, wanted);
    if topology.is_silent() && census.unclassified.is_empty() {
        return rundir::find_question(repo_root, wanted).map(Located::Legacy);
    }
    let legacy = legacy_candidates(repo_root, &census.committed, wanted);
    decide(
        wanted,
        &rundir::runs_root(repo_root),
        legacy,
        topology,
        &census.unclassified,
    )
}

fn decide(
    wanted: &str,
    runs_root: &Path,
    mut legacy: LegacyCandidates,
    mut topology: observe::OpenCandidates,
    unclassified: &[String],
) -> Result<Located, UpstrokeError> {
    let refused = |message: String| UpstrokeError::Refused { message };
    match (legacy.exact, topology.exact) {
        (Some(found), None) => return Ok(Located::Legacy(found)),
        (None, Some(open)) => return Ok(Located::Topology(open)),
        (Some(found), Some(open)) => {
            return Err(refused(format!(
                "`{wanted}` is the id of question {} in run {} and of question {} in run {}; an \
                 answer to one id cannot say which run it is for, so nothing was written",
                found.question_id, found.run_id, open.asked.question.id, open.run_id
            )));
        }
        (None, None) => {}
    }
    if let Some(settled) = topology
        .settled
        .iter()
        .find(|settled| settled.question_id.eq_ignore_ascii_case(wanted))
    {
        return Err(refused(no_longer_open(settled)));
    }
    let matched = legacy.prefixed.len() + topology.prefixed.len() + topology.settled.len();
    if matched == 1 && unclassified.is_empty() && topology.unread.is_empty() {
        if let Some(found) = legacy.prefixed.pop() {
            return Ok(Located::Legacy(found));
        }
        if let Some(open) = topology.prefixed.pop() {
            return Ok(Located::Topology(open));
        }
        if let Some(settled) = topology.settled.first() {
            return Err(refused(no_longer_open(settled)));
        }
    }
    let note = unread_note(unclassified, &topology.unread);
    let mut named: Vec<&str> = legacy
        .prefixed
        .iter()
        .map(|found| found.question_id.as_str())
        .collect();
    named.extend(
        topology
            .prefixed
            .iter()
            .map(|open| open.asked.question.id.as_str()),
    );
    named.extend(
        topology
            .settled
            .iter()
            .map(|settled| settled.question_id.as_str()),
    );
    Err(refused(match named.as_slice() {
        [] => format!(
            "no question with that id under {}{note}",
            runs_root.display()
        ),
        [only] if topology.unread.is_empty() => format!(
            "that prefix matches one question ({only}) among the runs that classified, and a \
             run that did not may hold another; use the full question id{note}"
        ),
        [only] => format!(
            "that prefix matches one question ({only}) among the runs that could be read, and a \
             run that could not may hold another; use the full question id{note}"
        ),
        several => format!(
            "that prefix matches {} questions ({}); use more characters{note}",
            several.len(),
            several.join(", ")
        ),
    }))
}

fn no_longer_open(settled: &observe::SettledQuestion) -> String {
    format!(
        "question {} of run {} is no longer open: the run's log has settled it, so an answer \
         now would be applied to nothing. Nothing was written",
        settled.question_id, settled.run_id
    )
}

fn unread_note(unclassified: &[String], unobserved: &[String]) -> String {
    let unclassified = match unclassified {
        [] => String::new(),
        [only] => format!(
            "; and `{only}` could not be classified, so whether a run committed there is \
             unknown — nothing has been deleted, and the next command classifies it again from \
             its own read"
        ),
        several => format!(
            "; and {} directories could not be classified ({}), so whether a run committed in \
             them is unknown — nothing has been deleted, and the next command classifies them \
             again from its own read",
            several.len(),
            several.join(", ")
        ),
    };
    let unobserved = match unobserved {
        [] => String::new(),
        [only] => format!(
            "; and the log of run `{only}` could not be read, so whether it holds a question \
             with that id is unknown"
        ),
        several => format!(
            "; and the logs of {} runs could not be read ({}), so whether one of them holds a \
             question with that id is unknown",
            several.len(),
            several.join(", ")
        ),
    };
    format!("{unclassified}{unobserved}")
}

fn answer_open(open: FoundOpen, reply: Reply) -> Result<Answered, UpstrokeError> {
    let question = &open.asked.question;
    let answers = open.paths.answers();
    if let Some(existing) = interaction::read_answer(&answers, &question.id)? {
        return Err(UpstrokeError::Refused {
            message: format!(
                "question {} was already answered ({}). Answers are applied once; if the task \
                 needs different guidance it will raise a new question.",
                question.id,
                describe(&existing)
            ),
        });
    }
    let answer = reply_answer(question, reply)?;
    observe::check_reply(&open.asked, &answer)?;
    std::fs::create_dir_all(&answers).map_err(|source| UpstrokeError::Io {
        path: answers.clone(),
        source,
    })?;
    let record = interaction::AnswerRecord::unattributed(answer);
    interaction::write_answer(&answers, &question.id, &record)?;
    let run_is_live = rundir::is_running(&open.paths.public);
    Ok(Answered {
        run_id: open.run_id.clone(),
        question_id: question.id.as_str().to_owned(),
        answer: record.answer,
        run_is_live,
    })
}

fn describe(answer: &Answer) -> String {
    match answer {
        Answer::Answered { text } => format!("\"{}\"", crate::util::head(text, 60)),
        Answer::Declined => "declined".to_owned(),
        Answer::Unanswered => "left parked".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Question, QuestionKind, TaskId};
    use crate::rundir::scratch_tree::ScratchTree;

    /// A scratch tree for one test, guarded by the token that authorises
    /// its deletion.
    ///
    /// The helper here built `temp_dir()/upstroke-answer-<tag>-<pid>` and
    /// pre-cleaned it with a discarded `remove_dir_all` before it had any
    /// claim on the name, then returned a root nothing reclaimed
    /// (`PR64-CLEANUP-003-SCRATCH-PRECLEAN`, `PR7-SCRATCH-FIXTURE-LEAK`).
    /// `acquire` refuses an occupied root rather than emptying it, keys on
    /// a ULID no recycled pid can supply, and reclaims on drop.
    ///
    /// **Bind the guard to a live local**: `let _ = scratch("x")` drops it
    /// at the end of that statement and deletes the fixture.
    fn scratch(tag: &str) -> ScratchTree {
        let parent = std::env::temp_dir();
        match crate::rundir::scratch_tree::acquire(&parent, tag) {
            Ok(tree) => tree,
            Err(refusal) => panic!(
                "a scratch tree for `{tag}` under {}: {refusal:?}",
                parent.display()
            ),
        }
    }

    fn seed(repo: &Path, run: &str, id: &str) {
        let public = rundir::public_dir(repo, run);
        let questions = public.join("questions");
        std::fs::create_dir_all(&questions).expect("questions dir");
        std::fs::write(
            public.join("events.jsonl"),
            format!(
                "{{\"ts\":\"2026-08-20T00:00:00Z\",\"event\":\"run_started\",\
                 \"data\":{{\"schema\":3,\"run_id\":\"{run}\"}}}}\n"
            ),
        )
        .expect("committed first line");
        let record = QuestionRecord::open(Question {
            id: QuestionId::from(id),
            kind: QuestionKind::Unblock,
            affected_tasks: vec![TaskId::from("t1")],
            context: "every rung failed on the same assertion".to_owned(),
            options: vec![
                "retry on frontier".to_owned(),
                interaction::GIVE_UP_OPTION.to_owned(),
            ],
        });
        interaction::write_question(&questions, &record).expect("write question");
    }

    #[test]
    fn an_answer_lands_where_the_engine_will_find_it() {
        let tree = scratch("basic");
        let repo = tree.path().join("repo");
        seed(&repo, "01RUN", "q-01ABC");

        let recorded = answer(
            &repo,
            "q-01A",
            Reply::Text("write it in src/widget.rs".to_owned()),
        )
        .expect("answer by prefix");
        assert_eq!(recorded.run_id, "01RUN");
        assert_eq!(recorded.question_id, "q-01ABC");
        assert!(!recorded.run_is_live, "nothing is driving this run");

        let answers = rundir::public_dir(&repo, "01RUN").join("answers");
        let read = interaction::read_answer(&answers, &QuestionId::from("q-01ABC"))
            .expect("read")
            .expect("present");
        assert_eq!(
            read,
            Answer::Answered {
                text: "write it in src/widget.rs".to_owned()
            }
        );
    }

    #[test]
    fn an_option_number_preserves_the_option_action() {
        let tree = scratch("option");
        let repo = tree.path().join("repo");
        seed(&repo, "01RUN", "q-1");
        let recorded = answer(&repo, "q-1", Reply::Option(2)).expect("answer");
        assert_eq!(recorded.answer, Answer::Declined);

        seed(&repo, "02RUN", "q-2");
        let err = answer(&repo, "q-2", Reply::Option(9)).expect_err("no option 9");
        assert!(err.to_string().contains("1-2"), "got: {err}");
        assert!(
            !err.to_string().contains("1..2"),
            "`1..2` is the range excluding 2 to anyone who writes Rust: {err}"
        );
    }

    #[test]
    fn declining_is_expressible_and_distinct_from_answering() {
        let tree = scratch("decline");
        let repo = tree.path().join("repo");
        seed(&repo, "01RUN", "q-1");
        let recorded = answer(&repo, "q-1", Reply::Decline).expect("decline");
        assert_eq!(recorded.answer, Answer::Declined);
    }

    #[test]
    fn an_empty_answer_is_refused_rather_than_written() {
        let tree = scratch("empty");
        let repo = tree.path().join("repo");
        seed(&repo, "01RUN", "q-1");
        let err = answer(&repo, "q-1", Reply::Text("   ".to_owned())).expect_err("refused");
        assert!(err.to_string().contains("--decline"), "got: {err}");
        assert!(
            !rundir::public_dir(&repo, "01RUN")
                .join("answers")
                .join("q-1.json")
                .exists(),
            "nothing written"
        );
    }

    #[test]
    fn a_question_is_answered_once() {
        let tree = scratch("twice");
        let repo = tree.path().join("repo");
        seed(&repo, "01RUN", "q-1");

        let questions = rundir::public_dir(&repo, "01RUN").join("questions");
        let path = questions.join("q-1.json");
        let mut record: QuestionRecord =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("read")).expect("parse");
        record.answer = Some(Answer::Answered {
            text: "already handled".to_owned(),
        });
        interaction::write_question(&questions, &record).expect("rewrite");

        let err = answer(&repo, "q-1", Reply::Text("again".to_owned())).expect_err("refused");
        assert!(err.to_string().contains("already answered"), "got: {err}");
        assert!(
            err.to_string().contains("already handled"),
            "says what: {err}"
        );
    }

    #[test]
    fn an_unknown_question_says_where_it_looked() {
        let tree = scratch("missing");
        let repo = tree.path().join("repo");
        seed(&repo, "01RUN", "q-1");
        let err = answer(&repo, "q-nope", Reply::Decline).expect_err("no such question");
        assert!(err.to_string().contains("no question"), "got: {err}");
    }

    #[test]
    fn showing_a_question_renders_its_options() {
        let tree = scratch("show");
        let repo = tree.path().join("repo");
        seed(&repo, "01RUN", "q-1");
        let rendered = show(&repo, "q-1").expect("show");
        assert!(rendered.contains("1) retry on frontier"), "{rendered}");
        assert!(rendered.contains("t1"), "names what parked: {rendered}");
    }

    fn legacy_at(run_id: &str, question_id: &str) -> FoundQuestion {
        FoundQuestion {
            run_id: run_id.to_owned(),
            public: std::path::PathBuf::from("/nonexistent").join(run_id),
            question_id: question_id.to_owned(),
        }
    }

    fn open_at(run_id: &str, question_id: &str) -> FoundOpen {
        FoundOpen {
            run_id: run_id.to_owned(),
            paths: rundir::RunPaths::from_parts(
                std::path::PathBuf::from("/nonexistent/public").join(run_id),
                std::path::PathBuf::from("/nonexistent/private").join(run_id),
            ),
            asked: observe::OpenAsked {
                question: Question {
                    id: QuestionId::from(question_id),
                    kind: QuestionKind::Unblock,
                    affected_tasks: vec![TaskId::from("t1")],
                    context: String::new(),
                    options: Vec::new(),
                },
                binds_an_agent: false,
            },
        }
    }

    fn settled_at(run_id: &str, question_id: &str) -> observe::SettledQuestion {
        observe::SettledQuestion {
            run_id: run_id.to_owned(),
            question_id: question_id.to_owned(),
        }
    }

    fn legacy(exact: Option<FoundQuestion>, prefixed: Vec<FoundQuestion>) -> LegacyCandidates {
        LegacyCandidates { exact, prefixed }
    }

    fn schema_4(
        exact: Option<FoundOpen>,
        prefixed: Vec<FoundOpen>,
        settled: Vec<observe::SettledQuestion>,
        unread: &[&str],
    ) -> observe::OpenCandidates {
        observe::OpenCandidates {
            exact,
            prefixed,
            settled,
            unread: unread.iter().map(|run_id| (*run_id).to_owned()).collect(),
        }
    }

    /// What `decide` chose, as a value a test can compare: the schema and the
    /// question, or the refusal's sentence.
    fn decided(
        wanted: &str,
        legacy: LegacyCandidates,
        topology: observe::OpenCandidates,
        unclassified: &[&str],
    ) -> Result<(&'static str, String), String> {
        let unclassified: Vec<String> = unclassified
            .iter()
            .map(|run_id| (*run_id).to_owned())
            .collect();
        match decide(
            wanted,
            Path::new("/repo/.upstroke/runs"),
            legacy,
            topology,
            &unclassified,
        ) {
            Ok(Located::Legacy(found)) => Ok(("legacy", found.question_id)),
            Ok(Located::Topology(open)) => Ok(("schema 4", open.asked.question.id.to_string())),
            Err(refused) => Err(refused.to_string()),
        }
    }

    #[test]
    fn a_prefix_is_accepted_only_when_both_schemas_and_every_unread_run_leave_one_candidate() {
        let refused = |result: Result<(&'static str, String), String>| match result {
            Ok(chosen) => panic!("chose {chosen:?}"),
            Err(message) => message,
        };

        let message = refused(decided(
            "q-1",
            legacy(None, vec![legacy_at("01A", "q-1A"), legacy_at("01B", "q-1B")]),
            schema_4(None, vec![open_at("01T", "q-1T")], Vec::new(), &[]),
            &[],
        ));
        assert!(
            message.contains(
                "that prefix matches 3 questions (q-1A, q-1B, q-1T); use more characters"
            ),
            "{message}"
        );
        let message = refused(decided(
            "q-1",
            legacy(None, vec![legacy_at("01A", "q-1A")]),
            schema_4(
                None,
                vec![open_at("01T", "q-1T"), open_at("01T", "q-1V")],
                Vec::new(),
                &[],
            ),
            &[],
        ));
        assert!(
            message.contains("that prefix matches 3 questions (q-1A, q-1T, q-1V)"),
            "{message}"
        );
        let message = refused(decided(
            "q-1",
            legacy(None, vec![legacy_at("01A", "q-1A")]),
            schema_4(None, vec![open_at("01T", "q-1T")], Vec::new(), &[]),
            &[],
        ));
        assert!(
            message.contains("that prefix matches 2 questions (q-1A, q-1T)"),
            "{message}"
        );

        assert_eq!(
            decided(
                "q-1",
                legacy(None, vec![legacy_at("01A", "q-1A")]),
                schema_4(None, Vec::new(), Vec::new(), &[]),
                &[],
            ),
            Ok(("legacy", "q-1A".to_owned()))
        );
        assert_eq!(
            decided(
                "q-1",
                legacy(None, Vec::new()),
                schema_4(None, vec![open_at("01T", "q-1T")], Vec::new(), &[]),
                &[],
            ),
            Ok(("schema 4", "q-1T".to_owned()))
        );

        let message = refused(decided(
            "q-1",
            legacy(None, Vec::new()),
            schema_4(None, vec![open_at("01T", "q-1T")], Vec::new(), &[]),
            &["01DIR"],
        ));
        assert!(
            message.contains(
                "that prefix matches one question (q-1T) among the runs that classified, and a \
                 run that did not may hold another; use the full question id; and `01DIR` could \
                 not be classified"
            ),
            "the census's unclassified directory reaches a schema-4 prefix: {message}"
        );
        let message = refused(decided(
            "q-1",
            legacy(None, vec![legacy_at("01A", "q-1A")]),
            schema_4(None, Vec::new(), Vec::new(), &["01RUN4"]),
            &[],
        ));
        assert!(
            message.contains(
                "that prefix matches one question (q-1A) among the runs that could be read, and \
                 a run that could not may hold another; use the full question id; and the log \
                 of run `01RUN4` could not be read"
            ),
            "a schema-4 log that could not be read reaches a legacy prefix: {message}"
        );

        let message = refused(decided(
            "q-1",
            legacy(None, vec![legacy_at("01A", "q-1A")]),
            schema_4(None, Vec::new(), vec![settled_at("01T", "q-1S")], &[]),
            &[],
        ));
        assert!(
            message.contains("that prefix matches 2 questions (q-1A, q-1S)"),
            "a settled question is still a candidate: {message}"
        );
        let message = refused(decided(
            "q-1",
            legacy(None, Vec::new()),
            schema_4(None, Vec::new(), vec![settled_at("01T", "q-1S")], &[]),
            &[],
        ));
        assert!(
            message.contains("question q-1S of run 01T is no longer open"),
            "{message}"
        );
    }

    #[test]
    fn a_full_id_answers_whatever_else_matches_or_went_unread() {
        assert_eq!(
            decided(
                "q-1A",
                legacy(Some(legacy_at("01A", "q-1A")), Vec::new()),
                schema_4(
                    None,
                    vec![open_at("01T", "q-1AT"), open_at("01T", "q-1AV")],
                    Vec::new(),
                    &["01RUN4"],
                ),
                &["01DIR"],
            ),
            Ok(("legacy", "q-1A".to_owned()))
        );
        assert_eq!(
            decided(
                "q-1T",
                legacy(
                    None,
                    vec![legacy_at("01A", "q-1TA"), legacy_at("01B", "q-1TB")]
                ),
                schema_4(Some(open_at("01T", "q-1T")), Vec::new(), Vec::new(), &[]),
                &["01DIR"],
            ),
            Ok(("schema 4", "q-1T".to_owned()))
        );
        let both = decided(
            "q-1",
            legacy(Some(legacy_at("01A", "q-1")), Vec::new()),
            schema_4(Some(open_at("01T", "q-1")), Vec::new(), Vec::new(), &[]),
            &[],
        );
        assert!(
            both.as_ref()
                .is_err_and(|message| message.contains("cannot say which run it is for")),
            "{both:?}"
        );
        let settled = decided(
            "q-1S",
            legacy(None, vec![legacy_at("01A", "q-1SA")]),
            schema_4(None, Vec::new(), vec![settled_at("01T", "q-1S")], &[]),
            &[],
        );
        assert!(
            settled
                .as_ref()
                .is_err_and(|message| message.contains("q-1S of run 01T is no longer open")),
            "{settled:?}"
        );
    }

    /// Where only legacy runs answer, the composed path refuses in the legacy
    /// finder's own sentences: checked against `rundir::find_question` itself for
    /// the two refusals a fixture can reach (no fixture classifies a directory
    /// `Indeterminate`; see `RunDirClass`).
    #[test]
    fn the_composed_refusals_are_the_legacy_finders_sentences_where_only_legacy_runs_answer() {
        let tree = scratch("composed");
        let repo = tree.path().join("repo");
        seed(&repo, "01RUN", "q-1A");
        seed(&repo, "02RUN", "q-1B");
        for wanted in ["q-1", "q-nope"] {
            let committed = census(&repo).committed;
            let composed = decide(
                wanted,
                &rundir::runs_root(&repo),
                legacy_candidates(&repo, &committed, wanted),
                schema_4(None, Vec::new(), Vec::new(), &[]),
                &[],
            )
            .err()
            .map(|refused| refused.to_string());
            let today = rundir::find_question(&repo, wanted)
                .err()
                .map(|refused| refused.to_string());
            assert!(composed.is_some(), "{wanted}: refused");
            assert_eq!(composed, today, "{wanted}");
        }
    }
}
