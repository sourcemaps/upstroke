//! Extended notes: `docs/internals/decide.md`

// Allowlist placement: the funnel section of `effects/allowlist.toml`, which
// carries this module's review clause.
#![allow(clippy::disallowed_methods)]
#![forbid(clippy::disallowed_types, clippy::disallowed_macros)]

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::agent::{AdapterSource, AgentAdapter, TaskRun};
use crate::engine::topology::observe::{self, Observed, OpenAsked};
use crate::error::UpstrokeError;
use crate::ir::{Answer, Question};
use crate::runner::invocation::InvocationId;
use crate::runner::{AgentId, Runner};
use crate::topology::events::{
    AttemptSettlement, SettlementTransition, TopologyEvent, TopologyEventBody,
};

pub const INPUT_BOUND: usize = 64 * 1024;
pub const MAX_PROPOSALS_PER_QUESTION: u32 = 2;
pub const DEFAULT_MAX_COST_USD: f64 = 5.0;
const PROPOSALS_DIR: &str = "proposals";

pub struct DecideOptions {
    pub repo_root: PathBuf,
    pub run_id: String,
    pub question_id: Option<String>,
    pub max_cost_usd: f64,
    pub pools_path: Option<PathBuf>,
}

#[derive(Debug)]
pub struct Decided {
    pub report: String,
    pub refused: Vec<String>,
}

impl Decided {
    pub fn outcome(&self) -> Result<(), UpstrokeError> {
        if self.refused.is_empty() {
            return Ok(());
        }
        Err(UpstrokeError::Refused {
            message: format!(
                "{} repl(ies) were refused ({}); each is saved with its reason",
                self.refused.len(),
                self.refused.join(", ")
            ),
        })
    }
}

struct Decider<'a> {
    adapter: &'a dyn AgentAdapter,
    agent: String,
    model: String,
    effort: crate::ir::Effort,
    timeout: std::time::Duration,
    credential: Option<PathBuf>,
}

fn refused(message: String) -> UpstrokeError {
    UpstrokeError::Refused { message }
}

pub fn proposals_dir(repo_root: &Path, run_id: &str) -> PathBuf {
    repo_root.join(".upstroke").join(PROPOSALS_DIR).join(run_id)
}

pub fn decide(
    opts: &DecideOptions,
    adapters: &dyn AdapterSource,
    runner: &dyn Runner,
) -> Result<Decided, UpstrokeError> {
    let observed = read_run(&opts.repo_root, &opts.run_id)?;
    let questions = chosen_questions(&observed, &opts.run_id, opts.question_id.as_deref())?;
    propose(opts, &observed, &questions, adapters, runner)
}

/// Asks the run's decider about each of `questions`, in order.
fn propose(
    opts: &DecideOptions,
    observed: &Observed,
    questions: &[OpenAsked],
    adapters: &dyn AdapterSource,
    runner: &dyn Runner,
) -> Result<Decided, UpstrokeError> {
    let Some(started) = observed.started() else {
        return Err(refused(format!(
            "run `{}` has no schema-4 run_started, so it cannot be read as one",
            opts.run_id
        )));
    };
    let Some(primary) = started.reviews.primary.clone() else {
        return Err(refused(format!(
            "run `{}` recorded no primary review binding (reviews.primary), so there is no \
             decider to ask; nothing else is chosen in its place",
            opts.run_id
        )));
    };
    let Some(adapter) = adapters.get(&primary.agent) else {
        return Err(refused(format!(
            "run `{}`'s primary reviewer `{}` has no built-in adapter",
            opts.run_id, primary.agent
        )));
    };
    let mut warnings = Vec::new();
    let config = crate::config::load_for_report(
        None,
        &opts.repo_root,
        opts.pools_path.as_deref(),
        &mut warnings,
    )?;
    let credential = crate::capacity::pool_for(&primary.agent, &config.pools)
        .and_then(|pool| pool.profile.as_ref())
        .map(PathBuf::from);
    let decider = Decider {
        adapter,
        agent: primary.agent.clone(),
        model: primary.model.clone(),
        effort: started.effort_policy.review,
        timeout: started
            .reviews
            .pass_timeout()
            .unwrap_or(crate::config::DEFAULT_REVIEW_PASS_TIMEOUT),
        credential,
    };
    let plan_text = read_bounded(&opts.repo_root.join(&started.plan_path));
    let dir = proposals_dir(&opts.repo_root, &opts.run_id);

    let mut out = String::new();
    let mut refusals = Vec::new();
    let mut spent = 0.0_f64;
    let mut unknown_cost = Vec::new();
    for (index, asked) in questions.iter().enumerate() {
        let id = asked.question.id.as_str().to_owned();
        if asked.binds_an_agent {
            let _ = writeln!(
                out,
                "{id}: skipped — it chooses which agent runs a task, and binding questions are \
                 left to a person; no agent was asked"
            );
            continue;
        }
        if spent >= opts.max_cost_usd {
            let left: Vec<&str> = questions
                .iter()
                .skip(index)
                .filter(|later| !later.binds_an_agent)
                .map(|later| later.question.id.as_str())
                .collect();
            let _ = writeln!(
                out,
                "stopped: this command's invocations have cost ${spent:.4}, which reaches the \
                 --max-cost-usd cap of ${:.4}; left without a proposal: {}",
                opts.max_cost_usd,
                left.join(", ")
            );
            break;
        }
        let Some(n) = next_number(&dir, &id) else {
            let _ = writeln!(
                out,
                "{id}: refused — it already has {MAX_PROPOSALS_PER_QUESTION} saved proposals \
                 under {}; no agent was asked",
                dir.display()
            );
            refusals.push(id.clone());
            continue;
        };
        let prompt = build_prompt(observed, asked, &plan_text);
        let ordinal = u32::try_from(index + 1).unwrap_or(u32::MAX);
        let asked_once = ask(
            &decider,
            runner,
            &opts.repo_root,
            &dir,
            &id,
            n,
            ordinal,
            &prompt,
        )?;
        match asked_once.cost_usd {
            Some(cost) => spent += cost,
            None => unknown_cost.push(id.clone()),
        }
        let verdict = asked_once
            .reply
            .as_ref()
            .map_err(Clone::clone)
            .and_then(|text| check_reply(&asked.question, text));
        let saved = save(
            &dir,
            &id,
            n,
            &SaveRecord {
                run_id: &opts.run_id,
                decider: &decider,
                verdict: &verdict,
                cost_usd: asked_once.cost_usd,
                raw: &asked_once.raw_path,
            },
        )?;
        render_one(
            &mut out,
            &decider,
            &asked.question,
            &verdict,
            asked_once.cost_usd,
            &saved,
        );
        if verdict.is_err() {
            refusals.push(id.clone());
        }
    }
    let _ = writeln!(out, "total cost: ${spent:.4}");
    if !unknown_cost.is_empty() {
        let _ = writeln!(
            out,
            "cost unknown for: {} (the agent reported none, so the total is a floor)",
            unknown_cost.join(", ")
        );
    }
    let _ = writeln!(
        out,
        "nothing was answered: a person applies a proposal with `upstroke answer <question-id>`"
    );
    Ok(Decided {
        report: out,
        refused: refusals,
    })
}

fn read_run(repo_root: &Path, run_id: &str) -> Result<Observed, UpstrokeError> {
    let public = crate::rundir::public_dir(repo_root, run_id);
    if !public.join(crate::rundir::EVENT_LOG).is_file() {
        return Err(refused(format!(
            "no run `{run_id}` under {} (the run id must be exact)",
            crate::rundir::runs_root(repo_root).display()
        )));
    }
    match observe::observe(repo_root, run_id) {
        Ok(Some(observed)) => Ok(observed),
        Ok(None) => Err(refused(format!(
            "run `{run_id}` is a schema 1-3 run; decide reads only schema-4 runs"
        ))),
        Err(error) => Err(refused(format!(
            "run `{run_id}` could not be read as a schema-4 run: {error}"
        ))),
    }
}

fn chosen_questions(
    observed: &Observed,
    run_id: &str,
    wanted: Option<&str>,
) -> Result<Vec<OpenAsked>, UpstrokeError> {
    let mut open = observed.open_questions();
    let order: Vec<String> = observed
        .raised_questions()
        .iter()
        .map(|(question, _)| question.id.as_str().to_owned())
        .collect();
    open.sort_by_key(|asked| {
        order
            .iter()
            .position(|id| id == asked.question.id.as_str())
            .unwrap_or(usize::MAX)
    });
    let Some(wanted) = wanted else {
        return Ok(open);
    };
    let upper = wanted.to_ascii_uppercase();
    if let Some(at) = open
        .iter()
        .position(|asked| asked.question.id.as_str().eq_ignore_ascii_case(wanted))
    {
        return Ok(vec![open.swap_remove(at)]);
    }
    let settled = |id: &str| {
        refused(format!(
            "question {id} of run {run_id} is no longer open: the run's log has settled it, so \
             an answer now would be applied to nothing. Nothing was written"
        ))
    };
    if let Some(id) = order.iter().find(|id| id.eq_ignore_ascii_case(wanted)) {
        return Err(settled(id));
    }
    let prefixed: Vec<usize> = open
        .iter()
        .enumerate()
        .filter(|(_, asked)| {
            asked
                .question
                .id
                .as_str()
                .to_ascii_uppercase()
                .starts_with(&upper)
        })
        .map(|(at, _)| at)
        .collect();
    let settled_prefixed: Vec<&String> = order
        .iter()
        .filter(|id| id.to_ascii_uppercase().starts_with(&upper))
        .filter(|id| {
            !open
                .iter()
                .any(|asked| asked.question.id.as_str() == id.as_str())
        })
        .collect();
    match (prefixed.as_slice(), settled_prefixed.as_slice()) {
        ([at], []) => Ok(vec![open.swap_remove(*at)]),
        ([], [id]) => Err(settled(id)),
        ([], []) => Err(refused(format!("no question with that id in run {run_id}"))),
        _ => Err(refused(format!(
            "that prefix matches {} questions in run {run_id}; use more characters",
            prefixed.len() + settled_prefixed.len()
        ))),
    }
}

/// Who raised a question, read from the event that opened it.
fn raised_by(event: &TopologyEvent) -> &'static str {
    match &event.body {
        TopologyEventBody::QuestionRaised { .. } => "the worker",
        TopologyEventBody::AttemptFinished { data } => {
            match (&data.settlement, &data.record.failure) {
                (
                    AttemptSettlement::Closed {
                        transition: SettlementTransition::Parked { .. },
                        ..
                    },
                    Some(failure),
                ) if failure.origin == crate::ladder::FailureOrigin::Reviewer => "a reviewer",
                _ => "the worker",
            }
        }
        _ => "integration",
    }
}

/// `text` cut to at most `INPUT_BOUND` bytes on a character boundary, and whether it was cut.
fn bounded(text: &str) -> (&str, bool) {
    if text.len() <= INPUT_BOUND {
        return (text, false);
    }
    let mut end = INPUT_BOUND;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (text.get(..end).unwrap_or_default(), true)
}

fn read_bounded(path: &Path) -> String {
    match crate::util::read_file_bounded(path) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(error) => format!(
            "(the plan file {} could not be read: {error})",
            path.display()
        ),
    }
}

fn section(prompt: &mut String, title: &str, text: &str) {
    let (kept, cut) = bounded(text);
    let _ = writeln!(prompt, "## {title}\n");
    prompt.push_str(kept);
    if cut {
        let _ = write!(
            prompt,
            "\n\n[cut here: this input was cut at {INPUT_BOUND} bytes of {}]",
            text.len()
        );
    }
    prompt.push_str("\n\n");
}

fn build_prompt(observed: &Observed, asked: &OpenAsked, plan_text: &str) -> String {
    let question = &asked.question;
    let raised = observed.raised_questions();
    let opener = raised.iter().find(|(frozen, _)| frozen.id == question.id);
    let by = opener.map_or("the worker", |(_, event)| raised_by(event));
    let key = opener.map(|(frozen, _)| frozen.key);

    let mut prompt = String::new();
    prompt.push_str(
        "You are asked to propose an answer to a question an upstroke run is parked on. You may \
         only read: do not edit, create or delete any file, and run nothing that writes. Nothing \
         you write will be applied; a person reads your proposal and decides. Giving up on the \
         task is never an available answer here.\n\n",
    );
    let mut asked_text = format!(
        "id: {}\nraised by: {by}\ncontext:\n{}\n",
        question.id, question.context
    );
    if by != "the worker" {
        let _ = writeln!(
            asked_text,
            "{by} judged this a person's decision rather than one it could make itself."
        );
    }
    if question.options.is_empty() {
        asked_text.push_str("options: none (choice must be null)\n");
    } else {
        asked_text.push_str("options:\n");
        for (at, option) in question.options.iter().enumerate() {
            let _ = writeln!(asked_text, "  {}) {option}", at + 1);
        }
    }
    section(&mut prompt, "The question", &asked_text);

    let task = key
        .and_then(|key| observed.registry().and_then(|registry| registry.get(key)))
        .map(|entry| {
            let task = entry.legacy_task();
            format!(
                "{} ({})\n\n{}\n\nacceptance:\n{}",
                task.title,
                task.id,
                task.body,
                task.acceptance.join("\n")
            )
        })
        .unwrap_or_else(|| "(the task is not in the run's registry)".to_owned());
    section(&mut prompt, "The task's current text", &task);

    let failure = key
        .and_then(|key| last_failure(observed.events(), key))
        .unwrap_or_else(|| "(no settled attempt of this task failed)".to_owned());
    section(&mut prompt, "The task's last settled attempt", &failure);

    let answers = key.map(|key| earlier_answers(observed.events(), key));
    section(
        &mut prompt,
        "The run's earlier answers for this task",
        answers
            .as_deref()
            .filter(|text| !text.is_empty())
            .unwrap_or("(none)"),
    );
    section(&mut prompt, "The plan file", plan_text);

    prompt.push_str(
        "## Your reply\n\nReply with exactly one fenced ```json block holding an object with \
         these six fields and nothing else outside it that is fenced:\n\
         - \"choice\": the 1-based number of the option you propose, or null when the question \
         has no options\n\
         - \"guidance\": text a person would give the task\n\
         - \"reasons\": a list of text\n\
         - \"confidence\": \"low\", \"medium\" or \"high\"\n\
         - \"needs_a_person\": true or false\n\
         - \"citations\": a list of text naming the files, lines or run facts you relied on\n",
    );
    prompt
}

fn last_failure(
    events: &[TopologyEvent],
    key: crate::topology::registry::TaskKey,
) -> Option<String> {
    events.iter().rev().find_map(|event| match &event.body {
        TopologyEventBody::AttemptFinished { data } if data.key == key => {
            data.record.failure.as_ref().map(|failure| {
                format!(
                    "attempt {} failed ({:?}, origin {}): {}{}",
                    data.attempt.0,
                    failure.kind,
                    match failure.origin {
                        crate::ladder::FailureOrigin::Worker => "worker",
                        crate::ladder::FailureOrigin::Reviewer => "reviewer",
                    },
                    failure.reason,
                    failure
                        .detail
                        .as_deref()
                        .map_or_else(String::new, |detail| format!("\n{detail}"))
                )
            })
        }
        _ => None,
    })
}

fn earlier_answers(events: &[TopologyEvent], key: crate::topology::registry::TaskKey) -> String {
    let mut text = String::new();
    for event in events {
        if let TopologyEventBody::QuestionAnswered { data } = &event.body {
            if data.key == key {
                let _ = writeln!(
                    text,
                    "{}: {:?}{}",
                    data.question.0,
                    data.answer,
                    data.text
                        .as_deref()
                        .map_or_else(String::new, |words| format!(" — \"{words}\""))
                );
            }
        }
    }
    text
}

struct Asked {
    reply: Result<String, String>,
    cost_usd: Option<f64>,
    raw_path: PathBuf,
}

#[expect(
    clippy::too_many_arguments,
    reason = "one call site; each is a distinct input"
)]
fn ask(
    decider: &Decider<'_>,
    runner: &dyn Runner,
    repo_root: &Path,
    dir: &Path,
    id: &str,
    n: u32,
    ordinal: u32,
    prompt: &str,
) -> Result<Asked, UpstrokeError> {
    std::fs::create_dir_all(dir).map_err(|source| UpstrokeError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    let profile = crate::review::profile_for(
        &decider.agent,
        &decider.model,
        &format!("decide-{}", decider.model),
        decider.effort,
    );
    let settings_path =
        decider
            .adapter
            .materialize_permissions(&profile, &[], dir, &format!("{id}.{n}-decide"))?;
    let task_run = TaskRun {
        prompt: prompt.to_owned(),
        profile,
        workspace: repo_root.to_path_buf(),
        gate_cmds: Vec::new(),
        resume_session: None,
        settings_path,
    };
    let command = decider
        .adapter
        .build(&task_run)?
        .stdin(decider.adapter.stdin_payload(&task_run).as_bytes().to_vec())
        .settings_file(task_run.settings_path.clone());
    let mut request = crate::runner::decide_request(
        command,
        task_run.workspace.clone(),
        AgentId::new(decider.adapter.id()),
        decider.timeout,
        InvocationId::Decide { ordinal },
    );
    if let Some(credential) = &decider.credential {
        request = request.with_credential_profile(credential.clone());
    }
    let raw_path = dir.join(format!("{id}.{n}.out"));
    let output = match runner.run_blocking(&request) {
        Ok(output) => output,
        Err(error) => {
            let reason = format!("the agent process failed: {}", UpstrokeError::from(error));
            write_new(&raw_path, reason.as_bytes())?;
            return Ok(Asked {
                reply: Err(reason),
                cost_usd: None,
                raw_path,
            });
        }
    };
    write_new(&raw_path, output.stdout.as_bytes())?;
    Ok(match decider.adapter.parse(&output) {
        Ok(outcome) => Asked {
            reply: Ok(outcome.detail.unwrap_or_default()),
            cost_usd: outcome.cost_usd,
            raw_path,
        },
        Err(error) => Asked {
            reply: Err(format!(
                "the adapter could not parse the agent's output: {error}"
            )),
            cost_usd: None,
            raw_path,
        },
    })
}

/// The one fenced `json` block of `text`, or why there is not exactly one.
fn fenced_json(text: &str) -> Result<&str, String> {
    let mut blocks = Vec::new();
    let mut rest = text;
    let mut offset = 0;
    while let Some(at) = rest.find("```json") {
        let start = offset + at + "```json".len();
        let after = text.get(start..).unwrap_or_default();
        let Some(end) = after.find("```") else {
            return Err("a fenced json block is never closed".to_owned());
        };
        blocks.push(after.get(..end).unwrap_or_default());
        offset = start + end + 3;
        rest = text.get(offset..).unwrap_or_default();
    }
    match blocks.as_slice() {
        [only] => Ok(only),
        [] => Err("the reply holds no fenced json block".to_owned()),
        several => Err(format!(
            "the reply holds {} fenced json blocks; exactly one is asked for",
            several.len()
        )),
    }
}

const FIELDS: [&str; 6] = [
    "choice",
    "guidance",
    "reasons",
    "confidence",
    "needs_a_person",
    "citations",
];

fn text_list(record: &Value, field: &str) -> Result<(), String> {
    match record.get(field).and_then(Value::as_array) {
        Some(items) if items.iter().all(Value::is_string) => Ok(()),
        _ => Err(format!("`{field}` is not a list of text")),
    }
}

/// The parsed record, if it passes every check.
fn check_reply(question: &Question, text: &str) -> Result<Value, String> {
    let block = fenced_json(text)?;
    let record: Value = serde_json::from_str(block)
        .map_err(|error| format!("the json block does not parse: {error}"))?;
    let Some(object) = record.as_object() else {
        return Err("the json block is not an object".to_owned());
    };
    for field in FIELDS {
        if !object.contains_key(field) {
            return Err(format!("the field `{field}` is missing"));
        }
    }
    if !record["guidance"].is_string() {
        return Err("`guidance` is not text".to_owned());
    }
    text_list(&record, "reasons")?;
    text_list(&record, "citations")?;
    if !matches!(
        record["confidence"].as_str(),
        Some("low" | "medium" | "high")
    ) {
        return Err("`confidence` is not \"low\", \"medium\" or \"high\"".to_owned());
    }
    if !record["needs_a_person"].is_boolean() {
        return Err("`needs_a_person` is not a bool".to_owned());
    }
    match &record["choice"] {
        Value::Null if question.options.is_empty() => {}
        Value::Null => {
            return Err(format!(
                "`choice` is null, but the question offers options 1-{}",
                question.options.len()
            ));
        }
        choice => {
            let number = choice
                .as_u64()
                .and_then(|number| usize::try_from(number).ok())
                .ok_or_else(|| "`choice` is not an option number".to_owned())?;
            match crate::interaction::answer_for_option(question, number) {
                None => {
                    return Err(format!(
                        "`choice` {number} is not one of the question's options ({})",
                        if question.options.is_empty() {
                            "it has none".to_owned()
                        } else {
                            format!("1-{}", question.options.len())
                        }
                    ));
                }
                Some(Answer::Answered { text })
                    if crate::interaction::interpret(question, &text) == Answer::Declined =>
                {
                    return Err(format!("`choice` {number} gives up on the task"));
                }
                Some(Answer::Answered { .. }) => {}
                Some(Answer::Declined | Answer::Unanswered) => {
                    return Err(format!(
                        "`choice` {number} gives up on the task, and giving up is never an \
                         answer a decider may propose"
                    ));
                }
            }
        }
    }
    Ok(record)
}

/// The next free proposal number for `id`, or `None` when it already has its cap.
fn next_number(dir: &Path, id: &str) -> Option<u32> {
    (1..=MAX_PROPOSALS_PER_QUESTION).find(|n| !dir.join(format!("{id}.{n}.json")).exists())
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), UpstrokeError> {
    let io = |source| UpstrokeError::Io {
        path: path.to_path_buf(),
        source,
    };
    let mut file = std::fs::File::create_new(path).map_err(io)?;
    file.write_all(bytes).map_err(io)
}

struct SaveRecord<'a> {
    run_id: &'a str,
    decider: &'a Decider<'a>,
    verdict: &'a Result<Value, String>,
    cost_usd: Option<f64>,
    raw: &'a Path,
}

fn save(dir: &Path, id: &str, n: u32, record: &SaveRecord<'_>) -> Result<PathBuf, UpstrokeError> {
    let (status, body) = match record.verdict {
        Ok(parsed) => ("accepted", ("record", parsed.clone())),
        Err(reason) => ("refused", ("reason", Value::String(reason.clone()))),
    };
    let mut document = json!({
        "run_id": record.run_id,
        "question_id": id,
        "agent": record.decider.agent,
        "model": record.decider.model,
        "effort": record.decider.effort,
        "status": status,
        "cost_usd": record.cost_usd,
        "raw_output": record.raw.to_string_lossy(),
    });
    if let Some(object) = document.as_object_mut() {
        object.insert(body.0.to_owned(), body.1);
    }
    let path = dir.join(format!("{id}.{n}.json"));
    let mut bytes = serde_json::to_vec_pretty(&document).map_err(|error| UpstrokeError::Parse {
        message: format!("{}: {error}", path.display()),
    })?;
    bytes.push(b'\n');
    write_new(&path, &bytes)?;
    Ok(path)
}

fn render_one(
    out: &mut String,
    decider: &Decider<'_>,
    question: &Question,
    verdict: &Result<Value, String>,
    cost_usd: Option<f64>,
    saved: &Path,
) {
    let _ = writeln!(
        out,
        "{}: decided by {}/{} at {} effort",
        question.id, decider.agent, decider.model, decider.effort
    );
    match verdict {
        Err(reason) => {
            let _ = writeln!(out, "  status: refused — {reason}");
        }
        Ok(record) => {
            let _ = writeln!(out, "  status: accepted");
            let choice = record["choice"].as_u64().map_or_else(
                || "none".to_owned(),
                |number| {
                    let text = usize::try_from(number)
                        .ok()
                        .and_then(|number| number.checked_sub(1))
                        .and_then(|at| question.options.get(at))
                        .map_or("", String::as_str);
                    format!("{number}) {text}")
                },
            );
            let _ = writeln!(out, "  choice: {choice}");
            let _ = writeln!(
                out,
                "  guidance: {}",
                record["guidance"].as_str().unwrap_or("")
            );
            let _ = writeln!(
                out,
                "  confidence: {}",
                record["confidence"].as_str().unwrap_or("")
            );
            let _ = writeln!(out, "  needs a person: {}", record["needs_a_person"]);
            for (field, label) in [("reasons", "reason"), ("citations", "citation")] {
                for item in record[field].as_array().into_iter().flatten() {
                    let _ = writeln!(out, "  {label}: {}", item.as_str().unwrap_or(""));
                }
            }
        }
    }
    let _ = writeln!(
        out,
        "  cost: {}",
        cost_usd.map_or_else(|| "unknown".to_owned(), |cost| format!("${cost:.4}"))
    );
    let _ = writeln!(out, "  saved: {}", saved.display());
}

#[cfg(test)]
mod tests;
