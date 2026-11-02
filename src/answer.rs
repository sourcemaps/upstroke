//! Extended notes: `docs/internals/answer.md`

// LEGACY-EFFECT: this module is in the frozen legacy section of
// `effects/allowlist.toml`, which carries its justification.
#![allow(clippy::disallowed_methods)]

use std::path::Path;

use crate::engine::topology::observe::{self, FoundOpen, QuestionLookup};
use crate::error::UpstrokeError;
use crate::interaction::{self, QuestionRecord};
use crate::ir::{Answer, Question, QuestionId};
use crate::rundir::{self, FoundQuestion};

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

fn locate(repo_root: &Path, wanted: &str) -> Result<Located, UpstrokeError> {
    let legacy = rundir::find_question(repo_root, wanted);
    let refused = |message: String| UpstrokeError::Refused { message };
    match (legacy, observe::find_open_question(repo_root, wanted)) {
        (Ok(found), QuestionLookup::Found(open)) => {
            if found.question_id.eq_ignore_ascii_case(wanted) {
                Ok(Located::Legacy(found))
            } else if open.asked.question.id.as_str().eq_ignore_ascii_case(wanted) {
                Ok(Located::Topology(open))
            } else {
                Err(refused(format!(
                    "that prefix matches {} and {}; use more characters",
                    found.question_id, open.asked.question.id
                )))
            }
        }
        (Ok(found), QuestionLookup::Ambiguous(ids)) => {
            if found.question_id.eq_ignore_ascii_case(wanted) {
                Ok(Located::Legacy(found))
            } else {
                Err(refused(format!(
                    "that prefix matches {} questions ({}, {}); use more characters",
                    ids.len() + 1,
                    found.question_id,
                    ids.join(", ")
                )))
            }
        }
        (Ok(found), QuestionLookup::Closed { .. } | QuestionLookup::Absent { .. }) => {
            Ok(Located::Legacy(found))
        }
        (Err(_), QuestionLookup::Found(open)) => Ok(Located::Topology(open)),
        (Err(_), QuestionLookup::Ambiguous(ids)) => Err(refused(format!(
            "that prefix matches {} open questions ({}); use more characters",
            ids.len(),
            ids.join(", ")
        ))),
        (
            Err(_),
            QuestionLookup::Closed {
                run_id,
                question_id,
            },
        ) => Err(refused(format!(
            "question {question_id} of run {run_id} is no longer open: the run's log has \
             settled it, so an answer now would be applied to nothing. Nothing was written"
        ))),
        (Err(error), QuestionLookup::Absent { unread }) if unread.is_empty() => Err(error),
        (Err(error), QuestionLookup::Absent { unread }) => Err(refused(format!(
            "{error}; and the schema-4 run(s) {} could not be read, so whether one of them \
             holds that question is unknown",
            unread.join(", ")
        ))),
    }
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
}
