//! Extended notes: `docs/internals/decide.md`

// Allowlist placement: the funnel section of `effects/allowlist.toml`, which
// carries this module's review clause.
#![allow(clippy::disallowed_methods)]
#![forbid(clippy::disallowed_types, clippy::disallowed_macros)]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Component, Path, PathBuf, Prefix};

use serde_json::{Value, json};

use crate::agent::{AdapterSource, AgentAdapter, TaskRun, claude};
use crate::capacity::Pool;
use crate::engine::topology::observe::{self, Observed, OpenAsked};
use crate::error::UpstrokeError;
use crate::ir::{Answer, Question, WorkerProfile};
use crate::runner::invocation::InvocationId;
use crate::runner::{AgentId, CommandSpec, Runner};
use crate::topology::events::{
    AttemptSettlement, RunStarted4, RunnerKind, SettlementTransition, TopologyEvent,
    TopologyEventBody,
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
    pub environment: Option<fn(&str) -> Option<OsString>>,
}

impl DecideOptions {
    /// Where the credential preflight reads the coordinator's environment: this process's own
    /// (as a run's preflight reads it) unless `environment` names a stand-in.
    fn environment(&self) -> fn(&str) -> Option<OsString> {
        self.environment
            .unwrap_or(crate::engine::topology::preflight::coordinator_environment)
    }
}

/// A promise every decide call has to keep. An adapter's word is not enough: `kept` checks
/// each one on the command actually built for the call, before it is started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Guarantee {
    /// The call reports what it cost in dollars, so `--max-cost-usd` can sum what was spent.
    CostReporting,
    /// The call stops at a dollar ceiling of its own: what is left of `--max-cost-usd`.
    PerCallCeiling,
    /// The call can fetch no web page and run no web search.
    NoWebAccess,
    /// The call can read nothing under the run's private directory.
    PrivateRunDataUnreadable,
}

impl Guarantee {
    const ALL: [Self; 4] = [
        Self::CostReporting,
        Self::PerCallCeiling,
        Self::NoWebAccess,
        Self::PrivateRunDataUnreadable,
    ];

    const fn named(self) -> &'static str {
        match self {
            Self::CostReporting => "cost reporting",
            Self::PerCallCeiling => "a per-call dollar ceiling",
            Self::NoWebAccess => "no web access",
            Self::PrivateRunDataUnreadable => "the run's private data being unreadable",
        }
    }
}

/// The guarantees a decide call through the adapter `id` can keep. Claude Code keeps all four
/// with flags and settings `kept` can see on the built command. Copilot and Codex report no
/// dollar cost (`Caps::cost_reporting` is false for both), so neither could be summed or
/// stopped at a ceiling, and nothing is claimed for them.
fn guarantees_of(id: &str) -> &'static [Guarantee] {
    if id == claude::ADAPTER_ID {
        &Guarantee::ALL
    } else {
        &[]
    }
}

/// The flags a decide call adds to the read-only profile: the call's own dollar ceiling, and no
/// MCP server but the ones these arguments name (there are none).
fn call_args(ceiling_usd: f64) -> Vec<String> {
    vec![
        "--max-budget-usd".to_owned(),
        ceiling_usd.to_string(),
        "--strict-mcp-config".to_owned(),
    ]
}

/// The read-only profile's settings, with reads of the run's private directory denied too.
fn call_settings(profile: &WorkerProfile, private_denials: &[String]) -> Value {
    let mut settings = claude::permission_settings(profile, &[]);
    if let Some(deny) = settings
        .pointer_mut("/permissions/deny")
        .and_then(Value::as_array_mut)
    {
        deny.extend(private_denials.iter().cloned().map(Value::String));
    }
    settings
}

/// Characters a path must not hold to be written into a permission rule exactly.
const PATTERN_CHARACTERS: [char; 9] = ['*', '?', '[', ']', '{', '}', '(', ')', '\\'];

/// `dir` in Claude Code's absolute rule spelling: `//`, then the path with `/` separators and
/// a Windows drive as its lower-case letter. `None` when it cannot be spelled exactly.
fn rule_path(dir: &Path) -> Option<String> {
    let mut spelled = String::from("/");
    let mut rooted = false;
    for component in dir.components() {
        match component {
            Component::Prefix(prefix) => match prefix.kind() {
                Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => {
                    spelled.push('/');
                    spelled.push(char::from(letter).to_ascii_lowercase());
                }
                _ => return None,
            },
            Component::RootDir => rooted = true,
            Component::Normal(name) => {
                let name = name.to_str()?;
                if name
                    .chars()
                    .any(|c| c.is_control() || PATTERN_CHARACTERS.contains(&c))
                {
                    return None;
                }
                spelled.push('/');
                spelled.push_str(name);
            }
            Component::CurDir | Component::ParentDir => return None,
        }
    }
    (rooted && dir.is_absolute() && spelled.len() > 1).then_some(spelled)
}

/// The deny rules that keep `private_dir` unreadable: the directory and everything under it,
/// as recorded and, where it resolves to another path, as resolved.
fn private_denials(private_dir: &str) -> Result<Vec<String>, String> {
    let recorded = PathBuf::from(private_dir);
    let mut spellings = vec![recorded.clone()];
    if let Ok(resolved) = std::fs::canonicalize(&recorded) {
        if resolved != recorded {
            spellings.push(resolved);
        }
    }
    let mut rules = Vec::new();
    for dir in &spellings {
        let Some(spelled) = rule_path(dir) else {
            return Err(format!(
                "its private directory `{}` cannot be written into a permission rule exactly (it \
                 must be an absolute, normalized, Unicode path with no pattern characters)",
                dir.display()
            ));
        };
        rules.push(format!("Read({spelled})"));
        rules.push(format!("Read({spelled}/**)"));
    }
    Ok(rules)
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
    private_denials: Vec<String>,
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
    let private_denials = admit(opts, started, adapter, &config.pools)?;
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
        private_denials,
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
        let call = prepare(
            &decider,
            &opts.repo_root,
            &dir.join(format!("{id}.{n}-decide.json")),
            &prompt,
            opts.max_cost_usd - spent,
        )?;
        let asked_once = ask(&decider, runner, &call, &dir, &id, n, ordinal)?;
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

const NOTHING_DONE: &str = "nothing was written and no agent was asked";

/// Refuses a run whose recorded reviewer environment a decide call cannot keep its promises in,
/// before anything is written or any agent asked. Returns the deny rules that keep the run's
/// private directory unreadable.
fn admit(
    opts: &DecideOptions,
    started: &RunStarted4,
    adapter: &dyn AgentAdapter,
    pools: &[Pool],
) -> Result<Vec<String>, UpstrokeError> {
    let run = &opts.run_id;
    if started.runner.kind != RunnerKind::Host {
        return Err(refused(format!(
            "run `{run}` was recorded on the container runner, and decide starts a reviewer on \
             this host only, outside the boundary the run's own reviewers ran in; {NOTHING_DONE}"
        )));
    }
    crate::engine::topology::preflight::refuse_unusable_profiles(
        pools,
        &started.runner,
        opts.environment(),
    )?;
    let keeps = guarantees_of(adapter.id());
    let missing: Vec<&str> = Guarantee::ALL
        .into_iter()
        .filter(|guarantee| !keeps.contains(guarantee))
        .map(Guarantee::named)
        .collect();
    if let Some((last, rest)) = missing.split_last() {
        let listed = if rest.is_empty() {
            (*last).to_owned()
        } else {
            format!("{} or {last}", rest.join(", "))
        };
        return Err(refused(format!(
            "run `{run}`'s primary reviewer `{}` cannot guarantee {listed}, and every decide call \
             must keep all of cost reporting, a per-call dollar ceiling, no web access and the \
             run's private data being unreadable; {NOTHING_DONE}",
            adapter.id()
        )));
    }
    if !(opts.max_cost_usd.is_finite() && opts.max_cost_usd > 0.0) {
        return Err(refused(format!(
            "--max-cost-usd {} is not a positive number of dollars, so no call could be given a \
             ceiling; {NOTHING_DONE}",
            opts.max_cost_usd
        )));
    }
    private_denials(&started.private_dir)
        .map_err(|reason| refused(format!("run `{run}`: {reason}; {NOTHING_DONE}")))
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

/// One decide call, built and checked. Nothing of it is written yet.
struct Call {
    task_run: TaskRun,
    command: CommandSpec,
    settings: Value,
}

/// Builds the call for `prompt` with a dollar ceiling of `ceiling_usd`, and refuses it unless
/// the command actually built carries no environment overlay and keeps every guarantee.
/// Writes nothing and starts nothing.
fn prepare(
    decider: &Decider<'_>,
    repo_root: &Path,
    settings_path: &Path,
    prompt: &str,
    ceiling_usd: f64,
) -> Result<Call, UpstrokeError> {
    let mut profile = crate::review::profile_for(
        &decider.agent,
        &decider.model,
        &format!("decide-{}", decider.model),
        decider.effort,
    );
    profile.extra_args = call_args(ceiling_usd);
    let settings = call_settings(&profile, &decider.private_denials);
    let task_run = TaskRun {
        prompt: prompt.to_owned(),
        profile,
        workspace: repo_root.to_path_buf(),
        gate_cmds: Vec::new(),
        resume_session: None,
        settings_path: Some(settings_path.to_path_buf()),
    };
    let command = decider
        .adapter
        .build(&task_run)?
        .stdin(decider.adapter.stdin_payload(&task_run).as_bytes().to_vec())
        .settings_file(task_run.settings_path.clone());
    let id = decider.adapter.id();
    let refuse = |reason: String| {
        refused(format!(
            "the command built for `{id}` {reason}, so it was not started; {NOTHING_DONE} for \
             this question"
        ))
    };
    if !command.env.is_empty() {
        let keys: Vec<&str> = command.env.iter().map(|(key, _)| key.as_str()).collect();
        return Err(refuse(format!(
            "carries an environment overlay ({}), and a decide call takes none",
            keys.join(", ")
        )));
    }
    let built = Built::read(
        &command,
        &settings,
        settings_path,
        ceiling_usd,
        &decider.private_denials,
    )
    .map_err(refuse)?;
    for guarantee in Guarantee::ALL {
        kept(guarantee, &built)
            .map_err(|reason| refuse(format!("does not keep {}: {reason}", guarantee.named())))?;
    }
    Ok(Call {
        task_run,
        command,
        settings,
    })
}

/// The flags a decide call is built with that take no value.
const BARE_FLAGS: [&str; 2] = ["-p", "--strict-mcp-config"];
/// The flags a decide call is built with that take one value each.
const VALUED_FLAGS: [&str; 8] = [
    "--output-format",
    "--model",
    "--permission-mode",
    "--setting-sources",
    "--effort",
    "--max-turns",
    "--settings",
    "--max-budget-usd",
];
/// The tools a decide call's settings may allow: reading, and nothing else.
const READ_TOOLS: [&str; 3] = ["Read", "Glob", "Grep"];

/// A built Claude Code command, read flag by flag, beside the settings written for it.
struct Built<'a> {
    flags: BTreeMap<&'a str, Option<&'a str>>,
    settings: &'a Value,
    settings_named: bool,
    ceiling_usd: f64,
    private_denials: &'a [String],
}

impl<'a> Built<'a> {
    /// Refuses a command that starts another program, or carries a flag no decide call is built
    /// with, a flag twice, or a value-taking flag with no value.
    fn read(
        command: &'a CommandSpec,
        settings: &'a Value,
        settings_path: &Path,
        ceiling_usd: f64,
        private_denials: &'a [String],
    ) -> Result<Self, String> {
        let program = Path::new(&command.program)
            .file_stem()
            .and_then(std::ffi::OsStr::to_str);
        if program != Some("claude") {
            return Err(format!(
                "starts `{}`, not Claude Code's CLI",
                command.program
            ));
        }
        let mut flags = BTreeMap::new();
        let mut args = command.args.iter();
        while let Some(arg) = args.next() {
            let flag = arg.as_str();
            let value = if BARE_FLAGS.contains(&flag) {
                None
            } else if VALUED_FLAGS.contains(&flag) {
                let Some(value) = args.next() else {
                    return Err(format!("ends with `{flag}` and no value"));
                };
                Some(value.as_str())
            } else {
                return Err(format!(
                    "carries `{flag}`, which no decide call is built with"
                ));
            };
            if flags.insert(flag, value).is_some() {
                return Err(format!("carries `{flag}` twice"));
            }
        }
        let named = settings_path.to_str();
        let settings_named = named.is_some()
            && flags.get("--settings") == Some(&named)
            && command.settings_file.as_deref() == Some(settings_path);
        Ok(Self {
            flags,
            settings,
            settings_named,
            ceiling_usd,
            private_denials,
        })
    }

    fn has(&self, flag: &str) -> Result<(), String> {
        match self.flags.get(flag) {
            Some(None) => Ok(()),
            _ => Err(format!("it does not carry `{flag}`")),
        }
    }

    fn value(&self, flag: &str) -> Result<&'a str, String> {
        match self.flags.get(flag) {
            Some(&Some(value)) => Ok(value),
            _ => Err(format!("it does not carry `{flag}`")),
        }
    }

    fn value_is(&self, flag: &str, wanted: &str) -> Result<(), String> {
        let value = self.value(flag)?;
        if value == wanted {
            Ok(())
        } else {
            Err(format!(
                "it carries `{flag} {value:?}`, not `{flag} {wanted:?}`"
            ))
        }
    }

    /// The settings the command names are the ones written for it, hold nothing but an allow
    /// and a deny list, and allow reading tools only.
    fn reads_only(&self) -> Result<(), String> {
        if !self.settings_named {
            return Err("its `--settings` does not name the settings written for it".to_owned());
        }
        let only = |value: &Value, keys: &[&str]| {
            value
                .as_object()
                .is_some_and(|object| object.keys().all(|key| keys.contains(&key.as_str())))
        };
        if !only(self.settings, &["permissions"])
            || !self
                .settings
                .get("permissions")
                .is_some_and(|permissions| only(permissions, &["allow", "deny"]))
        {
            return Err(format!(
                "its settings hold more than an allow and a deny list: {}",
                self.settings
            ));
        }
        let allow = self
            .settings
            .pointer("/permissions/allow")
            .and_then(Value::as_array)
            .ok_or_else(|| "its settings carry no allow list".to_owned())?;
        match allow
            .iter()
            .find(|rule| !rule.as_str().is_some_and(|rule| READ_TOOLS.contains(&rule)))
        {
            Some(rule) => Err(format!(
                "its settings allow {rule}, which is more than reading"
            )),
            None => Ok(()),
        }
    }

    fn denies<S: AsRef<str>>(&self, rules: &[S]) -> Result<(), String> {
        let deny = self
            .settings
            .pointer("/permissions/deny")
            .and_then(Value::as_array)
            .ok_or_else(|| "its settings carry no deny list".to_owned())?;
        match rules
            .iter()
            .map(AsRef::as_ref)
            .find(|rule| !deny.iter().any(|denied| denied.as_str() == Some(rule)))
        {
            Some(rule) => Err(format!("its settings do not deny `{rule}`")),
            None => Ok(()),
        }
    }
}

/// Whether `built` keeps `guarantee`, and if not, the first thing it lacks.
fn kept(guarantee: Guarantee, built: &Built<'_>) -> Result<(), String> {
    match guarantee {
        Guarantee::CostReporting => {
            built.has("-p")?;
            built.value_is("--output-format", "json")
        }
        Guarantee::PerCallCeiling => {
            let value = built.value("--max-budget-usd")?;
            match value.parse::<f64>() {
                Ok(usd) if usd.is_finite() && usd > 0.0 && usd <= built.ceiling_usd => Ok(()),
                _ => Err(format!(
                    "`--max-budget-usd {value}` is not a ceiling above $0 and at most the ${} \
                     left of --max-cost-usd",
                    built.ceiling_usd
                )),
            }
        }
        Guarantee::NoWebAccess => {
            built.value_is("--permission-mode", "dontAsk")?;
            built.value_is("--setting-sources", "")?;
            built.has("--strict-mcp-config")?;
            built.reads_only()?;
            built.denies(&["WebFetch", "WebSearch"])
        }
        Guarantee::PrivateRunDataUnreadable => {
            built.value_is("--setting-sources", "")?;
            built.reads_only()?;
            built.denies(&["Read(.upstroke/**)", "Read(**/.upstroke/**)"])?;
            if built.private_denials.is_empty() {
                return Err("no rule names the run's private directory".to_owned());
            }
            built.denies(built.private_denials)
        }
    }
}

fn ask(
    decider: &Decider<'_>,
    runner: &dyn Runner,
    call: &Call,
    dir: &Path,
    id: &str,
    n: u32,
    ordinal: u32,
) -> Result<Asked, UpstrokeError> {
    std::fs::create_dir_all(dir).map_err(|source| UpstrokeError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    if let Some(settings_path) = &call.task_run.settings_path {
        crate::util::write_json(settings_path, &call.settings)?;
    }
    let mut request = crate::runner::decide_request(
        call.command.clone(),
        call.task_run.workspace.clone(),
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
