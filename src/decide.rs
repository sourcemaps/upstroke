//! Extended notes: `docs/internals/decide.md`

// Allowlist placement: the funnel section of `effects/allowlist.toml`, which
// carries this module's review clause.
#![allow(clippy::disallowed_methods)]
#![forbid(clippy::disallowed_types, clippy::disallowed_macros)]

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Component, Path, PathBuf, Prefix};

use serde_json::{Value, json};

use crate::agent::{AdapterSource, AgentAdapter, ProcessOutput, TaskRun, claude};
use crate::capacity::Pool;
use crate::engine::topology::observe::{self, Observed, OpenAsked};
use crate::error::UpstrokeError;
use crate::events::FailureRecord;
use crate::ir::{Answer, OutcomeStatus, Question, WorkerProfile};
use crate::ladder::{FailureKind, FailureOrigin};
use crate::runner::invocation::InvocationId;
use crate::runner::{AgentId, CommandSpec, Runner};
use crate::topology::events::{
    AttemptSettlement, RunStarted4, RunnerKind, SettlementTransition, TopologyEvent,
    TopologyEventBody,
};
use crate::util::terminal::{TerminalLines, one_line};

pub const INPUT_BOUND: usize = 64 * 1024;
pub const DEFAULT_MAX_COST_USD: f64 = 5.0;
/// The repository's own directory, which the proposals directory is under.
const UPSTROKE_DIR: &str = ".upstroke";
const PROPOSALS_DIR: &str = "proposals";
/// The longest question id this command names a file after.
const MAX_ID_BYTES: usize = 128;

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
    /// The questions this command stopped before proposing an answer to: the call in flight
    /// when it stopped, and every question after it.
    pub left: Vec<String>,
}

impl Decided {
    pub fn outcome(&self) -> Result<(), UpstrokeError> {
        let mut failures = Vec::new();
        if !self.refused.is_empty() {
            failures.push(format!(
                "{} repl(ies) were refused ({}); each is saved with its reason",
                self.refused.len(),
                self.refused.join(", ")
            ));
        }
        if !self.left.is_empty() {
            failures.push(format!(
                "the command stopped with {} question(s) left without a proposal ({})",
                self.left.len(),
                self.left.join(", ")
            ));
        }
        if failures.is_empty() {
            return Ok(());
        }
        Err(UpstrokeError::Refused {
            message: one_line(failures.join("; ")),
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
    repo_root
        .join(UPSTROKE_DIR)
        .join(PROPOSALS_DIR)
        .join(run_id)
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
    if !plain_id(&opts.run_id) {
        return Err(refused(format!(
            "the run id `{}` is not a plain file-name component, so no proposals directory is \
             named after it; {NOTHING_DONE}",
            opts.run_id
        )));
    }
    let answered = answered(&observed.paths().answers(), questions)?;
    let askable = |asked: &OpenAsked| {
        let id = asked.question.id.as_str();
        !asked.binds_an_agent && plain_id(id) && !answered.contains_key(id)
    };
    if questions.iter().any(askable) {
        crate::engine::topology::preflight::certify_agent(
            &primary.agent,
            adapter,
            &config.pools,
            runner,
            &|what| {
                refused(format!(
                    "run `{}`'s recorded reviewer did not pass the pre-flight a run makes of it: \
                     {what}; {NOTHING_DONE}",
                    opts.run_id
                ))
            },
        )?;
    }
    let plan_text = recorded_plan(&opts.repo_root.join(&started.plan_path), &started.plan_hash);
    let dir = proposals_dir(&opts.repo_root, &opts.run_id);

    // Every line of the report is pushed whole through the terminal line builder: the agent's
    // reply, the run's question text and the file names are data inside a line, so none of them
    // can start a line of its own or reach the terminal as a control.
    let mut out = TerminalLines::default();
    let mut refusals = Vec::new();
    let mut left = Vec::new();
    let mut spent = 0.0_f64;
    let mut unsettled = None;
    let mut claimed = false;
    let mut called = BTreeSet::new();
    for (index, asked) in questions.iter().enumerate() {
        let id = asked.question.id.as_str().to_owned();
        if asked.binds_an_agent {
            out.push(format_args!(
                "{id}: skipped — it chooses which agent runs a task, and binding questions are \
                 left to a person; no agent was asked"
            ));
            continue;
        }
        if !plain_id(&id) {
            out.push(format_args!(
                "{id:?}: refused — its id is not a plain file-name component (1 to \
                 {MAX_ID_BYTES} ASCII letters, digits, `-` or `_`, and no device name), so no \
                 file is named after it; no agent was asked"
            ));
            refusals.push(id.clone());
            continue;
        }
        if let Some(answer) = answered.get(&id) {
            out.push(format_args!(
                "{id}: skipped — a person has answered it already ({}), and the run applies that \
                 answer; no agent was asked",
                described(answer)
            ));
            continue;
        }
        if !called.insert(id.clone()) {
            out.push(format_args!(
                "{id}: skipped — it was already asked in this command, and a question gets at \
                 most one call"
            ));
            continue;
        }
        // The questions this command has not answered once it stops at `from`, each named once.
        let rest = |from: usize| -> Vec<String> {
            let mut named = BTreeSet::new();
            questions
                .iter()
                .skip(from)
                .filter(|later| {
                    !later.binds_an_agent && !answered.contains_key(later.question.id.as_str())
                })
                .map(|later| later.question.id.as_str().to_owned())
                .filter(|later| named.insert(later.clone()))
                .collect()
        };
        let remaining = opts.max_cost_usd - spent;
        if remaining <= 0.0 {
            left = rest(index);
            out.push(format_args!(
                "stopped: this command's calls have spent ${spent:.4}, which reaches the \
                 --max-cost-usd allowance of ${:.4} for the run, so no further call is started; \
                 left without a proposal: {}",
                opts.max_cost_usd,
                listed(&left)
            ));
            break;
        }
        let names = Names::of(&dir, &id);
        let prompt = build_prompt(observed, asked, &plan_text);
        let call = match prepare(
            &decider,
            &opts.repo_root,
            &names.settings,
            &prompt,
            remaining,
        ) {
            Ok(call) => call,
            Err(error) if !claimed => return Err(error),
            Err(error) => {
                left = rest(index);
                stopped(&mut out, &error, &left);
                break;
            }
        };
        if !claimed {
            claim(&opts.repo_root, &opts.run_id, &dir)?;
            claimed = true;
        }
        let ordinal = u32::try_from(index + 1).unwrap_or(u32::MAX);
        let published = ask(&decider, runner, &call, &names, ordinal).and_then(|returned| {
            let verdict = match &returned {
                Returned::Settled { reply, .. } => check_reply(&asked.question, reply),
                Returned::Unsettled { reason, .. } => Err(reason.clone()),
            };
            let record = SaveRecord {
                run_id: &opts.run_id,
                decider: &decider,
                verdict: &verdict,
                returned: &returned,
                ceiling_usd: remaining,
            };
            save(&names, &record).map(|()| (returned, verdict))
        });
        let (returned, verdict) = match published {
            Ok(published) => published,
            Err(error) => {
                left = rest(index);
                stopped(&mut out, &error, &left);
                break;
            }
        };
        render_one(
            &mut out,
            &decider,
            &asked.question,
            &verdict,
            returned.settled_usd(),
            &names.proposal,
        );
        if verdict.is_err() {
            refusals.push(id.clone());
        }
        match returned {
            Returned::Settled { cost_usd, .. } => spent += cost_usd,
            Returned::Unsettled { reason, .. } => {
                left = rest(index + 1);
                out.push(format_args!(
                    "stopped: {id}'s call did not settle at a price ({reason}), so what it spent \
                     is unknown and no further call is started; left without a proposal: {}",
                    listed(&left)
                ));
                unsettled = Some(id);
                break;
            }
        }
    }
    out.push(format_args!("total cost: ${spent:.4}"));
    if let Some(id) = &unsettled {
        out.push(format_args!(
            "cost unknown for: {id} (its call did not settle at a price, so the total is a floor)"
        ));
    }
    if claimed {
        out.push(format_args!(
            "claimed: {}; decide asks a run once, so a later decide of this run writes nothing \
             and asks no agent",
            dir.display()
        ));
    }
    out.push(format_args!(
        "nothing was answered: a person applies a proposal with `upstroke answer <question-id>`"
    ));
    Ok(Decided {
        report: out.into_string(),
        refused: refusals,
        left,
    })
}

/// `ids` joined for a report line, or `none`.
fn listed(ids: &[String]) -> String {
    if ids.is_empty() {
        "none".to_owned()
    } else {
        ids.join(", ")
    }
}

/// The report's line for a command that stopped on `error` once the run was claimed.
fn stopped(out: &mut TerminalLines, error: &UpstrokeError, left: &[String]) {
    out.push(format_args!(
        "stopped: {error}; the run stays claimed and no further call is started; left without a \
         proposal: {}",
        listed(left)
    ));
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
    if crate::rundir::is_running(&crate::rundir::public_dir(&opts.repo_root, run)) {
        return Err(refused(format!(
            "run `{run}` is being driven: an engine holds its lock (or whether one does cannot be \
             told), and decide proposes answers only for a run nothing is driving, so the run's \
             own questions are not read while it may be settling them; {NOTHING_DONE}"
        )));
    }
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

/// The questions of `questions` a person has answered already, each with its answer: an answer
/// `upstroke answer` published in the run's answers directory `answers` that the run's log has
/// not settled yet. Read for every question this command could ask, before anything is written
/// or any agent asked, so a question the run will apply a person's answer to is never asked.
fn answered(
    answers: &Path,
    questions: &[OpenAsked],
) -> Result<BTreeMap<String, Answer>, UpstrokeError> {
    let mut answered = BTreeMap::new();
    for asked in questions {
        let id = asked.question.id.as_str();
        if asked.binds_an_agent || !plain_id(id) || answered.contains_key(id) {
            continue;
        }
        let read =
            crate::interaction::read_answer(answers, &asked.question.id).map_err(|error| {
                refused(format!(
                    "whether question {id} was answered cannot be told: {error}; {NOTHING_DONE}"
                ))
            })?;
        if let Some(answer) = read {
            answered.insert(id.to_owned(), answer);
        }
    }
    Ok(answered)
}

/// A person's answer, as the report names it.
fn described(answer: &Answer) -> String {
    match answer {
        Answer::Answered { text } => format!("\"{text}\""),
        Answer::Declined => "declined".to_owned(),
        Answer::Unanswered => "left parked".to_owned(),
    }
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

/// Who raised a question: the task's worker, or the coordinator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RaisedBy {
    /// The worker asked for a person: its attempt settled parked on a `NeedsHuman` failure of
    /// worker origin.
    Worker,
    /// The coordinator parked the task for a person: on a reviewer's verdict, once the task's
    /// ladder or deferrals ran out, to approve a spend, at a task's admission, or when
    /// integration verification could not finish.
    Coordinator,
}

impl RaisedBy {
    /// Read from the event that opened the question.
    fn of(event: &TopologyEvent) -> Self {
        let asked_by_worker = |failure: &FailureRecord| {
            failure.kind == FailureKind::NeedsHuman && failure.origin == FailureOrigin::Worker
        };
        match &event.body {
            TopologyEventBody::AttemptFinished { data }
                if matches!(
                    &data.settlement,
                    AttemptSettlement::Closed {
                        transition: SettlementTransition::Parked { .. },
                        ..
                    }
                ) && data.record.failure.as_ref().is_some_and(asked_by_worker) =>
            {
                Self::Worker
            }
            _ => Self::Coordinator,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Worker => "the worker",
            Self::Coordinator => "the coordinator",
        }
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

/// The plan file's text for the prompt, forwarded only when its bytes hash to the run's recorded
/// `plan_hash` (`ir::content_hash`, the hash the run froze the plan under); otherwise a line
/// saying why it is not, so a plan edited since the run started never reaches the reviewer as
/// the run's own.
fn recorded_plan(path: &Path, plan_hash: &str) -> String {
    match crate::util::read_file_bounded(path) {
        Ok(bytes) if crate::ir::content_hash(&bytes) == plan_hash => {
            String::from_utf8_lossy(&bytes).into_owned()
        }
        Ok(bytes) => format!(
            "(the plan file {} is not forwarded: its bytes hash to {}, not to the {plan_hash} \
             the run recorded, so it is not the plan this run froze)",
            path.display(),
            crate::ir::content_hash(&bytes)
        ),
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

/// The question as the prompt states it: its id, who raised it (`None` where no event of the
/// run's log is found to have opened it), its context and its options.
fn question_text(question: &Question, raised_by: Option<RaisedBy>) -> String {
    let by = raised_by.map_or(
        "unknown (no event of the run's log opened it)",
        RaisedBy::label,
    );
    let mut text = format!(
        "id: {}\nraised by: {by}\ncontext:\n{}\n",
        question.id, question.context
    );
    if raised_by == Some(RaisedBy::Coordinator) {
        text.push_str(
            "The coordinator raised this question itself; the task's worker did not ask it.\n",
        );
    }
    if question.options.is_empty() {
        text.push_str("options: none (choice must be null)\n");
    } else {
        text.push_str("options:\n");
        for (at, option) in question.options.iter().enumerate() {
            let _ = writeln!(text, "  {}) {option}", at + 1);
        }
    }
    text
}

fn build_prompt(observed: &Observed, asked: &OpenAsked, plan_text: &str) -> String {
    let question = &asked.question;
    let raised = observed.raised_questions();
    let opener = raised.iter().find(|(frozen, _)| frozen.id == question.id);
    let raised_by = opener.map(|(_, event)| RaisedBy::of(event));
    let key = opener.map(|(frozen, _)| frozen.key);

    let mut prompt = String::new();
    prompt.push_str(
        "You are asked to propose an answer to a question an upstroke run is parked on. You may \
         only read: do not edit, create or delete any file, and run nothing that writes. Nothing \
         you write will be applied; a person reads your proposal and decides. Giving up on the \
         task is never an available answer here.\n\n",
    );
    section(
        &mut prompt,
        "The question",
        &question_text(question, raised_by),
    );

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
         these six fields and nothing else outside it that is fenced. Its opening ```json and \
         its closing ``` are each a line of their own:\n\
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

/// What one call returned, once its raw output is saved.
#[derive(Debug, PartialEq)]
enum Returned {
    /// A clean, successful, priced call: the process exited 0 within its time and output
    /// limits, the adapter read a completed outcome, and it reported a finite cost of $0 or
    /// more. The reply's text, and the price the call settles at.
    Settled { reply: String, cost_usd: f64 },
    /// Every other outcome a call returns: why it does not settle, and any cost it reported,
    /// which is recorded and never counted.
    Unsettled {
        reason: String,
        cost_usd: Option<f64>,
    },
}

impl Returned {
    /// The price a settled call is counted at.
    const fn settled_usd(&self) -> Option<f64> {
        match self {
            Self::Settled { cost_usd, .. } => Some(*cost_usd),
            Self::Unsettled { .. } => None,
        }
    }

    /// Any cost the call reported, settled or not.
    const fn reported_usd(&self) -> Option<f64> {
        match self {
            Self::Settled { cost_usd, .. } => Some(*cost_usd),
            Self::Unsettled { cost_usd, .. } => *cost_usd,
        }
    }
}

/// How an adapter's outcome status reads in a refusal.
const fn status_named(status: OutcomeStatus) -> &'static str {
    match status {
        OutcomeStatus::Completed => "completed",
        OutcomeStatus::AgentError => "an agent error",
        OutcomeStatus::Timeout => "a timeout",
        OutcomeStatus::RateLimited => "rate limited",
    }
}

/// Reads what a call returned: `Settled` only for a clean, successful, priced call.
fn settle(adapter: &dyn AgentAdapter, output: &ProcessOutput) -> Returned {
    let parsed = adapter.parse(output);
    let reported = parsed.as_ref().ok().and_then(|outcome| outcome.cost_usd);
    let unsettled = |reason: String| Returned::Unsettled {
        reason,
        cost_usd: reported,
    };
    if output.timed_out {
        return unsettled("the call timed out".to_owned());
    }
    if output.output_limited {
        return unsettled("the call's output exceeded its limit".to_owned());
    }
    if output.code != Some(0) {
        return unsettled(match output.code {
            Some(code) => format!("the agent exited with status {code}"),
            None => "the agent was ended by a signal".to_owned(),
        });
    }
    let outcome = match parsed {
        Ok(outcome) => outcome,
        Err(error) => {
            return unsettled(format!(
                "the adapter could not parse the agent's output: {error}"
            ));
        }
    };
    if outcome.status != OutcomeStatus::Completed {
        return unsettled(format!(
            "the adapter read the call as {}",
            status_named(outcome.status)
        ));
    }
    match outcome.cost_usd {
        Some(cost) if cost.is_finite() && cost >= 0.0 => Returned::Settled {
            reply: outcome.detail.unwrap_or_default(),
            cost_usd: cost,
        },
        Some(cost) => unsettled(format!(
            "the call reported a cost of {cost}, which is no price"
        )),
        None => unsettled("the call reported no cost".to_owned()),
    }
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
    built
        .value_is("--model", &decider.model)
        .and_then(|()| built.value_is("--effort", &decider.effort.to_string()))
        .map_err(|reason| {
            refuse(format!(
                "is not the recorded reviewer's, {}/{} at {} effort: {reason}",
                decider.agent, decider.model, decider.effort
            ))
        })?;
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

/// Starts one call of the claimed run: writes its settings file new, starts it through the
/// Runner, saves its raw output new, and reads what it returned. A Runner error is a returned
/// outcome too, saved in place of the raw output, and never settles.
fn ask(
    decider: &Decider<'_>,
    runner: &dyn Runner,
    call: &Call,
    names: &Names,
    ordinal: u32,
) -> Result<Returned, UpstrokeError> {
    if let Some(settings_path) = &call.task_run.settings_path {
        write_new(settings_path, &pretty(&call.settings, settings_path)?)?;
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
    let output = match runner.run_blocking(&request) {
        Ok(output) => output,
        Err(error) => {
            let reason = format!("the agent process failed: {}", UpstrokeError::from(error));
            write_new(&names.raw, reason.as_bytes())?;
            return Ok(Returned::Unsettled {
                reason,
                cost_usd: None,
            });
        }
    };
    write_new(&names.raw, output.stdout.as_bytes())?;
    Ok(settle(decider.adapter, &output))
}

/// A fence: with an info string after it, a line that opens a block; alone, one that closes it.
const FENCE: &str = "```";
/// The info string of the block a reply answers in.
const REPLY_INFO: &str = "json";

/// The body of the one fenced block of `text` when that block is a closed `json` block, or why
/// the reply is refused. A fence is a whole line, with nothing beside it but whitespace (a `\r`
/// included): outside a block, "```" and an info string holding no backtick open one, and inside,
/// "```" alone closes it. A fence written within a line opens and closes nothing, so the JSON may
/// mention one in a string (a JSON string holds no raw line break, so it is never a line of its
/// own), and a block is read whole, a "```json" line inside it included. Every block is counted,
/// whatever its info string (none included) and whether it is closed or not: one left open runs
/// to the end of the reply, as Markdown reads it. A reply is accepted only when it holds exactly
/// one block and that block is a closed `json` block; a refusal says how many blocks it holds.
fn fenced_json(text: &str) -> Result<&str, String> {
    // Every block, in order: whether it is `json`, and its body once it is closed.
    let mut blocks: Vec<(bool, Option<&str>)> = Vec::new();
    // The block the walk is in, if any: whether it is `json`, and where its body starts.
    let mut open: Option<(bool, usize)> = None;
    let mut end = 0;
    for line in text.split_inclusive('\n') {
        let start = end;
        end += line.len();
        let bare = line.trim();
        match open {
            None => {
                open = bare
                    .strip_prefix(FENCE)
                    .filter(|info| !info.contains('`'))
                    .map(|info| (info.trim() == REPLY_INFO, end));
            }
            Some((json, body)) if bare == FENCE => {
                blocks.push((json, Some(text.get(body..start).unwrap_or_default())));
                open = None;
            }
            Some(_) => {}
        }
    }
    if let Some((json, _)) = open {
        blocks.push((json, None));
    }
    match blocks.as_slice() {
        [(true, Some(body))] => Ok(body),
        [(true, None)] => {
            Err("the reply holds 1 fenced block, a json block that is never closed".to_owned())
        }
        [(false, _)] => {
            Err("the reply holds 1 fenced block, and it is not a json block".to_owned())
        }
        [] => Err("the reply holds no fenced block; one json block is asked for".to_owned()),
        several => Err(format!(
            "the reply holds {} fenced blocks; exactly one, a json block, is asked for",
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

/// Whether `id` is a plain file-name component on every platform this command runs on: 1 to
/// `MAX_ID_BYTES` ASCII letters, digits, `-` or `_` (so no separator, no `.`, no `..`), and not
/// a Windows device name.
fn plain_id(id: &str) -> bool {
    let upper = id.to_ascii_uppercase();
    let device = ["CON", "PRN", "AUX", "NUL"].contains(&upper.as_str())
        || (upper.len() == 4
            && (upper.starts_with("COM") || upper.starts_with("LPT"))
            && upper
                .bytes()
                .last()
                .is_some_and(|last| last.is_ascii_digit()));
    !id.is_empty()
        && id.len() <= MAX_ID_BYTES
        && !device
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

/// What a directory of the proposals chain must keep from every user but its owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Privacy {
    /// No other user may write it, so none can add, rename or replace what is in it:
    /// `.upstroke`, which the run created and whose other contents other commands read.
    Closed,
    /// No other user may read, write or enter it: `.upstroke/proposals` and the run's own
    /// directory, which decide creates owner-only.
    OwnerOnly,
}

/// Unix: the directory belongs to the repository root's owner, and its mode grants no other
/// user what `privacy` withholds.
#[cfg(unix)]
fn owned_and_private(
    metadata: &std::fs::Metadata,
    repo_root: &Path,
    privacy: Privacy,
) -> Result<(), String> {
    use std::os::unix::fs::MetadataExt as _;
    let owner = std::fs::metadata(repo_root)
        .map_err(|error| format!("cannot be checked against the repository's owner: {error}"))?
        .uid();
    if metadata.uid() != owner {
        return Err(format!(
            "belongs to uid {}, not to the repository's owner (uid {owner})",
            metadata.uid()
        ));
    }
    let mode = metadata.mode() & 0o777;
    let (withheld, granted) = match privacy {
        Privacy::Closed => (0o022, "writable by a user other than its owner"),
        Privacy::OwnerOnly => (0o077, "open to a user other than its owner"),
    };
    if mode & withheld != 0 {
        return Err(format!("is {granted} (mode {mode:03o})"));
    }
    Ok(())
}

/// Windows: the directory is no reparse point (a junction or a directory symlink). std reads
/// no owner or access list there, so those are the ones the repository's own directory passes
/// down.
#[cfg(windows)]
fn owned_and_private(
    metadata: &std::fs::Metadata,
    _repo_root: &Path,
    _privacy: Privacy,
) -> Result<(), String> {
    use std::os::windows::fs::MetadataExt as _;
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
    if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err("is a reparse point".to_owned());
    }
    Ok(())
}

/// Whether the directory at `path` is one decide writes under: a real directory, read without
/// following a link, that `owned_and_private` admits at `privacy`.
fn trusted_dir(path: &Path, repo_root: &Path, privacy: Privacy) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("`{}` cannot be read: {error}", path.display()))?;
    if !metadata.file_type().is_dir() {
        return Err(format!(
            "`{}` is not a directory (a link or a file stands at its name)",
            path.display()
        ));
    }
    owned_and_private(&metadata, repo_root, privacy)
        .map_err(|reason| format!("`{}` {reason}", path.display()))
}

/// Unix: narrows a directory decide has just created to its owner. Elsewhere, nothing.
#[cfg(unix)]
fn make_owner_only(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn make_owner_only(_dir: &Path) -> std::io::Result<()> {
    Ok(())
}

/// Syncs the directory that holds `dir`, so the entry naming `dir` is durable.
fn sync_parent(dir: &Path) -> Result<(), String> {
    match dir.parent() {
        Some(parent) => crate::util::fsync_dir(parent)
            .map_err(|error| format!("`{}` could not be synced: {error}", parent.display())),
        None => Ok(()),
    }
}

/// A refusal before the run's directory is created.
fn unclaimed(reason: &str) -> UpstrokeError {
    refused(format!(
        "the proposals directory is not one decide can trust or make durable: {reason}; the run \
         was not claimed, and no agent was asked"
    ))
}

/// Makes sure `dir`, a directory above the run's, is there and trusted at `privacy`: creates it
/// owner-only if it is missing, checks it whoever created it, and syncs its parent whoever
/// created it, so its entry is durable before anything is created under it.
fn ensure_dir(repo_root: &Path, dir: &Path, privacy: Privacy) -> Result<(), UpstrokeError> {
    match std::fs::create_dir(dir) {
        Ok(()) => make_owner_only(dir).map_err(|error| {
            unclaimed(&format!(
                "`{}` was created but could not be made private: {error}",
                dir.display()
            ))
        })?,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => {
            return Err(unclaimed(&format!(
                "`{}` could not be created: {error}",
                dir.display()
            )));
        }
    }
    trusted_dir(dir, repo_root, privacy).map_err(|reason| unclaimed(&reason))?;
    sync_parent(dir).map_err(|reason| unclaimed(&reason))
}

/// Claims the run, once, immediately before its first call: `.upstroke` and
/// `.upstroke/proposals` are made sure of (`ensure_dir`), and then the run's own directory `dir`
/// is created exclusively. Whatever stands at its name already, an earlier decide's directory
/// complete, partial or of an older format, or anything else, refuses the command with nothing
/// written. Once created the directory is never removed: a failure after that leaves the claim
/// in place, so the run is not asked again.
fn claim(repo_root: &Path, run_id: &str, dir: &Path) -> Result<(), UpstrokeError> {
    let top = repo_root.join(UPSTROKE_DIR);
    ensure_dir(repo_root, &top, Privacy::Closed)?;
    ensure_dir(repo_root, &top.join(PROPOSALS_DIR), Privacy::OwnerOnly)?;
    match std::fs::create_dir(dir) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(refused(format!(
                "run `{run_id}` was claimed already: `{}` exists, and decide asks a run once, by \
                 creating that directory, so an earlier decide's directory (complete, partial or \
                 of an older format) or anything else at that name ends this one; \
                 {NOTHING_DONE}",
                dir.display()
            )));
        }
        Err(error) => {
            return Err(unclaimed(&format!(
                "`{}` could not be created: {error}",
                dir.display()
            )));
        }
    }
    make_owner_only(dir)
        .map_err(|error| format!("it could not be made private: {error}"))
        .and_then(|()| trusted_dir(dir, repo_root, Privacy::OwnerOnly))
        .and_then(|()| sync_parent(dir))
        .map_err(|reason| {
            refused(format!(
                "run `{run_id}` is claimed: `{}` was created, and it stays, so a later decide of \
                 the run writes nothing and asks no agent; but {reason}, so no agent was asked",
                dir.display()
            ))
        })
}

/// How a saved record names the file `name` of run `run_id`'s directory: its path from the
/// repository root, each component a plain name and `/` between them on every platform. It
/// names the same file wherever the repository is checked out and whichever system reads it,
/// where an absolute path would name the checkout that wrote it, in that system's spelling.
fn reference(run_id: &str, name: &str) -> String {
    format!("{UPSTROKE_DIR}/{PROPOSALS_DIR}/{run_id}/{name}")
}

/// The files one question's call creates in the run's directory, each named after the question's
/// plain id. An id holds no `.`, so the three suffixes keep every question's files apart.
struct Names {
    /// The question's id.
    id: String,
    /// `<id>.settings.json`: the call's permission settings.
    settings: PathBuf,
    /// `<id>.out`: the agent's raw output, or why its process failed.
    raw: PathBuf,
    raw_name: String,
    /// `<id>.json`: the proposal.
    proposal: PathBuf,
}

impl Names {
    fn of(dir: &Path, id: &str) -> Self {
        let raw_name = format!("{id}.out");
        Self {
            id: id.to_owned(),
            settings: dir.join(format!("{id}.settings.json")),
            raw: dir.join(&raw_name),
            raw_name,
            proposal: dir.join(format!("{id}.json")),
        }
    }
}

fn pretty(value: &Value, path: &Path) -> Result<Vec<u8>, UpstrokeError> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|error| UpstrokeError::Parse {
        message: format!("{}: {error}", path.display()),
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// Creates `path` new (`File::create_new`, which never replaces a file), writes `bytes`, and
/// makes the file and its name durable.
fn create_durably(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = std::fs::File::create_new(path)?;
    file.write_all(bytes)?;
    crate::util::fsync_file(&file)?;
    match path.parent() {
        Some(parent) => crate::util::fsync_dir(parent),
        None => Ok(()),
    }
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), UpstrokeError> {
    create_durably(path, bytes).map_err(|source| UpstrokeError::Io {
        path: path.to_path_buf(),
        source,
    })
}

struct SaveRecord<'a> {
    run_id: &'a str,
    decider: &'a Decider<'a>,
    verdict: &'a Result<Value, String>,
    returned: &'a Returned,
    /// The call's own `--max-budget-usd`: what was left of the allowance when it started.
    ceiling_usd: f64,
}

/// Creates the question's proposal new: accepted with its record, or refused with its reason.
fn save(names: &Names, record: &SaveRecord<'_>) -> Result<(), UpstrokeError> {
    let (status, body) = match record.verdict {
        Ok(parsed) => ("accepted", ("record", parsed.clone())),
        Err(reason) => ("refused", ("reason", Value::String(reason.clone()))),
    };
    let mut document = json!({
        "run_id": record.run_id,
        "question_id": names.id,
        "agent": record.decider.agent,
        "model": record.decider.model,
        "effort": record.decider.effort,
        "status": status,
        "cost_usd": record.returned.reported_usd(),
        "settled": record.returned.settled_usd().is_some(),
        "ceiling_usd": record.ceiling_usd,
        "raw_output": reference(record.run_id, &names.raw_name),
    });
    if let Some(object) = document.as_object_mut() {
        object.insert(body.0.to_owned(), body.1);
    }
    let bytes = pretty(&document, &names.proposal)?;
    write_new(&names.proposal, &bytes)
}

/// One question's lines of the report. The reply's fields, the refusal's reason and the option's
/// text are each data inside a line `out` builds, so none can start a line or reach the terminal
/// as a control; the saved record keeps them as they were.
fn render_one(
    out: &mut TerminalLines,
    decider: &Decider<'_>,
    question: &Question,
    verdict: &Result<Value, String>,
    cost_usd: Option<f64>,
    saved: &Path,
) {
    out.push(format_args!(
        "{}: decided by {}/{} at {} effort",
        question.id, decider.agent, decider.model, decider.effort
    ));
    match verdict {
        Err(reason) => out.push(format_args!("  status: refused — {reason}")),
        Ok(record) => {
            out.push(format_args!("  status: accepted"));
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
            out.push(format_args!("  choice: {choice}"));
            out.push(format_args!(
                "  guidance: {}",
                record["guidance"].as_str().unwrap_or("")
            ));
            out.push(format_args!(
                "  confidence: {}",
                record["confidence"].as_str().unwrap_or("")
            ));
            out.push(format_args!(
                "  needs a person: {}",
                record["needs_a_person"]
            ));
            for (field, label) in [("reasons", "reason"), ("citations", "citation")] {
                for item in record[field].as_array().into_iter().flatten() {
                    out.push(format_args!("  {label}: {}", item.as_str().unwrap_or("")));
                }
            }
        }
    }
    out.push(format_args!(
        "  cost: {}",
        cost_usd.map_or_else(|| "unknown".to_owned(), |cost| format!("${cost:.4}"))
    ));
    out.push(format_args!("  saved: {}", saved.display()));
}

#[cfg(test)]
mod tests;
