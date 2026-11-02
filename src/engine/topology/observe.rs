//! Extended notes: `docs/internals/engine/topology/observe.md`

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::error::UpstrokeError;
use crate::interaction::Sleeper;
use crate::ir::{Question, TaskId};
use crate::rundir::{self, RunPaths};
use crate::topology::events::{RunStarted4, RunnerKind, TopologyEvent, TopologyEventBody};
use crate::topology::fold::{OpenQuestion, QuestionOrigin, TopologyFold};
use crate::topology::registry::{TaskKey, TaskRegistry};
use crate::topology::schema::{
    MAX_READABLE_SCHEMA, ReaderSelection, probe_header, select_for_schema,
};
use crate::util::terminal::TerminalLines;

use super::report::{TopologyReport, outcome_label};

const STABLE_READ_ATTEMPTS: usize = 8;

pub struct Observed {
    run_id: String,
    log: PathBuf,
    paths: RunPaths,
    torn_tail: usize,
    running: bool,
    events: Vec<TopologyEvent>,
    fold: TopologyFold,
}

pub fn is_topology_log(bytes: &[u8], log: &Path) -> Result<bool, UpstrokeError> {
    let Ok(header) = probe_header(bytes) else {
        return Ok(false);
    };
    match select_for_schema(header.schema, MAX_READABLE_SCHEMA) {
        Ok(ReaderSelection::Topology) => Ok(true),
        Ok(ReaderSelection::Legacy { .. }) => Ok(false),
        Err(refusal) => Err(UpstrokeError::Refused {
            message: format!("{refusal} ({})", log.display()),
        }),
    }
}

pub fn observe(repo_root: &Path, run_id: &str) -> Result<Option<Observed>, UpstrokeError> {
    let public = rundir::public_dir(repo_root, run_id);
    let log = public.join(rundir::EVENT_LOG);
    let Ok(probe) = crate::util::read_file_bounded(&log) else {
        return Ok(None);
    };
    if !is_topology_log(&probe, &log)? {
        return Ok(None);
    }
    let (bytes, running) = stable_read(&log, &public)?;
    observe_bytes(run_id, &public, &bytes, running).map(Some)
}

pub fn observe_bytes(
    run_id: &str,
    public: &Path,
    bytes: &[u8],
    running: bool,
) -> Result<Observed, UpstrokeError> {
    let log = public.join(rundir::EVENT_LOG);
    let committed = committed_len(bytes);
    let (paths, events, fold) = checked_replay(run_id, public, &log, &bytes[..committed])?;
    Ok(Observed {
        run_id: run_id.to_owned(),
        log,
        paths,
        torn_tail: bytes.len() - committed,
        running,
        events,
        fold,
    })
}

fn committed_len(bytes: &[u8]) -> usize {
    bytes
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |last| last + 1)
}

fn read_log(log: &Path) -> Result<Vec<u8>, UpstrokeError> {
    crate::util::read_file_bounded(log).map_err(|source| UpstrokeError::Io {
        path: log.to_path_buf(),
        source,
    })
}

fn stable_read(log: &Path, public: &Path) -> Result<(Vec<u8>, bool), UpstrokeError> {
    for _ in 0..STABLE_READ_ATTEMPTS {
        let held_before = rundir::is_running(public);
        let first = read_log(log)?;
        let second = read_log(log)?;
        let held_after = rundir::is_running(public);
        let kept = committed_len(&first);
        if held_before == held_after && second.get(..kept) == first.get(..kept) {
            return Ok((second, held_after));
        }
    }
    Err(UpstrokeError::Refused {
        message: format!(
            "{} kept changing under its committed lines, or its run changed hands, across {} \
             reads; a log only grows at its end, so nothing is reported from it. Retry once the \
             transition settles",
            log.display(),
            STABLE_READ_ATTEMPTS
        ),
    })
}

fn checked_replay(
    run_id: &str,
    public: &Path,
    log: &Path,
    committed: &[u8],
) -> Result<(RunPaths, Vec<TopologyEvent>, TopologyFold), UpstrokeError> {
    let refused = |message: String| UpstrokeError::EventLog {
        path: log.to_path_buf(),
        message,
    };
    let events = TopologyFold::parse_log(committed).map_err(|error| refused(error.to_string()))?;
    let Some(TopologyEventBody::RunStarted { data: started }) =
        events.first().map(|event| &event.body)
    else {
        return Err(refused(
            "the log's first committed line is not a schema-4 `run_started`".to_owned(),
        ));
    };
    if started.run_id != run_id {
        return Err(refused(format!(
            "run_started id `{}` does not match directory `{run_id}`",
            started.run_id
        )));
    }
    let paths = RunPaths::from_parts(public.to_path_buf(), PathBuf::from(&started.private_dir));
    let inputs = super::entry::frozen_inputs(&paths, started)?;
    let fold = TopologyFold::replay(inputs, &events).map_err(|error| refused(error.to_string()))?;
    Ok((paths, events, fold))
}

impl Observed {
    #[must_use]
    pub fn paths(&self) -> &RunPaths {
        &self.paths
    }

    #[must_use]
    pub fn running(&self) -> bool {
        self.running
    }

    #[must_use]
    pub fn torn_tail(&self) -> usize {
        self.torn_tail
    }

    #[must_use]
    pub fn events(&self) -> &[TopologyEvent] {
        &self.events
    }

    #[must_use]
    pub fn started(&self) -> Option<&RunStarted4> {
        self.fold.started()
    }

    #[must_use]
    pub fn registry(&self) -> Option<&TaskRegistry> {
        self.fold.registry()
    }

    #[must_use]
    pub fn finished(&self) -> bool {
        self.fold.finished().is_some()
    }

    pub fn report(&self) -> Result<TopologyReport, UpstrokeError> {
        TopologyReport::derive(&self.run_id, &self.fold, &self.events)
    }

    #[must_use]
    pub fn display_id(&self, key: TaskKey) -> String {
        self.registry()
            .and_then(|registry| registry.get(key))
            .map_or_else(
                || format!("k{}", key.0),
                |entry| entry.display_id.to_string(),
            )
    }

    #[must_use]
    pub fn open_questions(&self) -> Vec<OpenAsked> {
        self.fold
            .open_questions()
            .map(|open| open.values().map(|asked| self.asked(asked)).collect())
            .unwrap_or_default()
    }

    #[must_use]
    pub fn raised(&self, id: &str) -> bool {
        self.events.iter().any(|event| {
            matches!(&event.body, TopologyEventBody::QuestionRaised { data }
                if data.question.id.0.eq_ignore_ascii_case(id))
        })
    }

    fn asked(&self, open: &OpenQuestion) -> OpenAsked {
        let frozen = &open.question;
        OpenAsked {
            question: Question {
                id: frozen.id.clone(),
                kind: frozen.kind,
                affected_tasks: vec![TaskId(self.display_id(frozen.key))],
                context: frozen.context.clone(),
                options: frozen.options.clone(),
            },
            binds_an_agent: open.origin == QuestionOrigin::Admission && open.binding.is_some(),
        }
    }

    pub fn render(&self) -> Result<String, UpstrokeError> {
        let report = self.report()?;
        let mut rendered = report.render();
        let mut out = TerminalLines::default();
        out.push(format_args!(
            "log: schema 4, {} committed line(s) in {}",
            self.events.len(),
            self.log.display()
        ));
        if self.running {
            out.push(format_args!(
                "state: running now (another process holds this run)"
            ));
        } else if report.outcome.is_none() {
            out.push(format_args!(
                "state: interrupted — this run stopped without finishing. Continue it with:"
            ));
            out.push(format_args!("    upstroke resume {}", self.run_id));
        }
        if self.torn_tail > 0 {
            out.push(format_args!(
                "warning: the log ends in {} byte(s) after its last commit marker; they are not \
                 an event, status leaves them where they are, and the next write command on this \
                 run settles them",
                self.torn_tail
            ));
        }
        let open = self.open_questions();
        if !open.is_empty() {
            out.push(format_args!("waiting on {} answer(s):", open.len()));
            for asked in &open {
                out.push(format_args!("    upstroke answer {}", asked.question.id));
            }
        }
        out.push(format_args!(
            "transcripts: {}",
            self.paths.private.display()
        ));
        rendered.push_str(&out.into_string());
        Ok(rendered)
    }

    #[must_use]
    pub fn describe(&self, event: &TopologyEvent) -> String {
        let detail = match &event.body {
            TopologyEventBody::RunStarted { data } => format!(
                " run {} (schema {}, {} runner)",
                data.run_id,
                data.schema,
                runner_kind(data.runner.kind)
            ),
            TopologyEventBody::RunResumed { data } => {
                format!(" incarnation {}", data.incarnation)
            }
            TopologyEventBody::AttemptStarted { data } => format!(
                " task {} attempt {} rung {} ({} {})",
                self.display_id(data.key),
                data.attempt.0,
                data.rung,
                data.binding.agent,
                data.binding.model
            ),
            TopologyEventBody::AttemptFinished { data } => format!(
                " task {} attempt {}{}",
                self.display_id(data.key),
                data.attempt.0,
                if data.record.failure.is_some() {
                    ": failed"
                } else {
                    ""
                }
            ),
            TopologyEventBody::AttemptInterrupted { data } => format!(
                " task {} attempt {}",
                self.display_id(data.key),
                data.attempt.0
            ),
            TopologyEventBody::TaskMerged { data } => format!(
                " {} ({})",
                data.merged_sha.0,
                data.satisfies
                    .iter()
                    .map(|key| self.display_id(*key))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            TopologyEventBody::QuestionRaised { data } => format!(
                " {} for task {}",
                data.question.id.0,
                self.display_id(data.question.key)
            ),
            TopologyEventBody::QuestionAnswered { data } => format!(
                " {} for task {}",
                data.question.0,
                self.display_id(data.key)
            ),
            TopologyEventBody::RunFinished { data } => {
                format!(" {}", outcome_label(&data.outcome))
            }
            _ => String::new(),
        };
        format!("{} {}{detail}", event.ts, event.body.kind())
    }
}

const fn runner_kind(kind: RunnerKind) -> &'static str {
    match kind {
        RunnerKind::Host => "host",
        RunnerKind::Container => "container",
    }
}

pub struct OpenAsked {
    pub question: Question,
    pub binds_an_agent: bool,
}

pub struct FoundOpen {
    pub run_id: String,
    pub paths: RunPaths,
    pub asked: OpenAsked,
}

pub enum QuestionLookup {
    Found(FoundOpen),
    Closed { run_id: String, question_id: String },
    Ambiguous(Vec<String>),
    Absent { unread: Vec<String> },
}

pub fn find_open_question(repo_root: &Path, wanted: &str) -> QuestionLookup {
    let wanted_upper = wanted.to_ascii_uppercase();
    let mut exact: Option<FoundOpen> = None;
    let mut by_prefix: Vec<FoundOpen> = Vec::new();
    let mut closed: Option<(String, String)> = None;
    let mut unread: Vec<String> = Vec::new();
    for run_id in rundir::list_runs(repo_root) {
        let observed = match observe(repo_root, &run_id) {
            Ok(Some(observed)) => observed,
            Ok(None) => continue,
            Err(_) => {
                unread.push(run_id);
                continue;
            }
        };
        for asked in observed.open_questions() {
            let id = asked.question.id.0.clone();
            let found = FoundOpen {
                run_id: run_id.clone(),
                paths: observed.paths().clone(),
                asked,
            };
            if id.eq_ignore_ascii_case(wanted) {
                exact = Some(found);
            } else if id.to_ascii_uppercase().starts_with(&wanted_upper) {
                by_prefix.push(found);
            }
        }
        if closed.is_none() && observed.raised(wanted) {
            closed = Some((run_id.clone(), wanted.to_owned()));
        }
    }
    if let Some(found) = exact {
        return QuestionLookup::Found(found);
    }
    if by_prefix.len() == 1 && unread.is_empty() {
        if let Some(found) = by_prefix.pop() {
            return QuestionLookup::Found(found);
        }
    }
    if by_prefix.len() > 1 {
        return QuestionLookup::Ambiguous(
            by_prefix
                .iter()
                .map(|found| found.asked.question.id.0.clone())
                .collect(),
        );
    }
    if let Some((run_id, question_id)) = closed {
        return QuestionLookup::Closed {
            run_id,
            question_id,
        };
    }
    QuestionLookup::Absent { unread }
}

pub fn check_reply(asked: &OpenAsked, answer: &crate::ir::Answer) -> Result<(), UpstrokeError> {
    let crate::ir::Answer::Answered { text } = answer else {
        return Ok(());
    };
    if !asked.binds_an_agent || asked.question.options.iter().any(|option| option == text) {
        return Ok(());
    }
    Err(UpstrokeError::Refused {
        message: format!(
            "question {} asks which agent runs task {}, and `{text}` is none of its {} option(s); \
             a binding is chosen only through an option the run froze, so answer it with \
             --option N. Nothing was written",
            asked.question.id,
            asked
                .question
                .affected_tasks
                .first()
                .map_or_else(String::new, ToString::to_string),
            asked.question.options.len()
        ),
    })
}

pub fn follow(
    repo_root: &Path,
    run_id: &str,
    sleeper: &dyn Sleeper,
    poll: Duration,
    max_idle_polls: u32,
    emit: &mut dyn FnMut(&str),
) -> Result<(), UpstrokeError> {
    let mut printed = 0_usize;
    let mut idle = 0_u32;
    loop {
        let Some(observed) = observe(repo_root, run_id)? else {
            return Err(UpstrokeError::Refused {
                message: format!(
                    "run `{run_id}`'s log no longer reads as schema 4, so following it as one \
                     stopped"
                ),
            });
        };
        let fresh = observed.events().get(printed..).unwrap_or_default();
        if fresh.is_empty() {
            if observed.finished() && !observed.running() {
                return Ok(());
            }
            if observed.running() {
                idle = 0;
            } else {
                idle += 1;
                if idle > max_idle_polls {
                    return Ok(());
                }
            }
            sleeper.sleep(poll);
            continue;
        }
        idle = 0;
        for event in fresh {
            emit(&observed.describe(event));
        }
        printed = observed.events().len();
        if observed.finished() && !observed.running() {
            return Ok(());
        }
    }
}
