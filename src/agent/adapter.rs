//! Extended notes: `docs/internals/agent/adapter.md`

#![forbid(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::capacity::PoolKind;
use crate::error::UpstrokeError;
use crate::ir::{Effort, Outcome, WorkerProfile};
use crate::runner::invocation::InvocationId;
use crate::runner::{AgentId, CommandSpec, ExecutionRole, ProbeTarget, Runner, RunnerRequest};

use super::proc::ProcessOutput;
use super::{claude, codex, copilot};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthState {
    Authenticated,
    NotAuthenticated,
    Unknown,
}

impl std::fmt::Display for AuthState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Authenticated => "signed in",
            Self::NotAuthenticated => {
                "NOT signed in — log in with the vendor's own CLI before running"
            }
            Self::Unknown => "auth state could not be determined",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discovery {
    pub auth: AuthState,
    pub models: Vec<String>,
    pub shape: Option<PoolKind>,
    pub notes: Vec<String>,
}

impl Discovery {
    pub fn unknown() -> Self {
        Self {
            auth: AuthState::Unknown,
            models: Vec::new(),
            shape: None,
            notes: Vec::new(),
        }
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Caps {
    pub version: String,
    pub json_output: bool,
    pub session_resume: bool,
    pub cost_reporting: bool,
    pub read_only_mode: bool,
    pub acp: bool,
    pub model_list: bool,
}

#[derive(Debug, Clone)]
pub struct TaskRun {
    pub prompt: String,
    pub profile: WorkerProfile,
    pub workspace: PathBuf,
    pub gate_cmds: Vec<String>,
    pub resume_session: Option<String>,
    pub settings_path: Option<PathBuf>,
}

#[must_use]
pub fn probe_workspace() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

pub fn probe_request(
    agent: &str,
    command: CommandSpec,
    ordinal: u32,
    timeout: Duration,
) -> Result<RunnerRequest, UpstrokeError> {
    let agent = AgentId::new(agent);
    Ok(RunnerRequest {
        command,
        workspace: probe_workspace(),
        role: ExecutionRole::Probe(ProbeTarget::Agent(agent.clone())),
        timeout,
        agent: Some(agent.clone()),
        invocation: InvocationId::probe(ProbeTarget::Agent(agent), ordinal)?,
    })
}

pub trait AgentAdapter: Send + Sync {
    fn id(&self) -> &'static str;
    fn probe(&self, runner: &dyn Runner) -> Result<Caps, UpstrokeError>;
    fn build(&self, run: &TaskRun) -> Result<CommandSpec, UpstrokeError>;
    fn parse(&self, out: &ProcessOutput) -> Result<Outcome, UpstrokeError>;

    fn discover(&self, _runner: &dyn Runner, _caps: &Caps) -> Result<Discovery, UpstrokeError> {
        Ok(Discovery::unknown())
    }

    fn stdin_payload<'a>(&self, run: &'a TaskRun) -> &'a str {
        &run.prompt
    }

    fn materialize_permissions(
        &self,
        _profile: &WorkerProfile,
        _gate_cmds: &[String],
        _dir: &std::path::Path,
        _stem: &str,
    ) -> Result<Option<PathBuf>, UpstrokeError> {
        Ok(None)
    }
}

pub trait AdapterSource {
    fn get(&self, id: &str) -> Option<&dyn AgentAdapter>;

    fn shared(&self) -> Option<Arc<dyn AdapterSource + Send + Sync>> {
        None
    }
}

pub struct BuiltinAdapters;

impl AdapterSource for BuiltinAdapters {
    fn get(&self, id: &str) -> Option<&dyn AgentAdapter> {
        by_id(id).map(|a| a as &dyn AgentAdapter)
    }

    fn shared(&self) -> Option<Arc<dyn AdapterSource + Send + Sync>> {
        Some(Arc::new(Self))
    }
}

pub static ADAPTERS: &[&dyn AgentAdapter] = &[
    &claude::ClaudeCodeAdapter,
    &copilot::CopilotAdapter,
    &codex::CodexAdapter,
];

pub fn by_id(id: &str) -> Option<&'static dyn AgentAdapter> {
    ADAPTERS.iter().copied().find(|a| a.id() == id)
}

pub(crate) fn missing_effort_levels(help: &str) -> Vec<Effort> {
    let mut block = String::new();
    let mut collecting = false;
    for line in help.lines() {
        if !collecting {
            if line.contains("--effort") {
                collecting = true;
                block.push_str(line);
                block.push('\n');
            }
            continue;
        }
        if line.trim_start().starts_with('-') {
            break;
        }
        block.push_str(line);
        block.push('\n');
    }
    if !collecting {
        return Effort::ALL.to_vec();
    }

    let advertised: Vec<Effort> = block
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter_map(Effort::parse)
        .collect();
    Effort::ALL
        .into_iter()
        .filter(|effort| !advertised.contains(effort))
        .collect()
}

pub(crate) fn advertises_flag(help: &str, flag: &str) -> bool {
    help.split(|character: char| character.is_whitespace() || character == ',')
        .map(|token| token.split(['=', ':']).next().unwrap_or(token))
        .any(|name| name == flag)
}

pub fn looks_rate_limited(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    [
        "usage limit",
        "rate limit",
        "rate_limit",
        "limit reached",
        "limit exceeded",
        "overloaded",
        "quota exceeded",
        "insufficient credits",
        "out of credits",
        "premium request",
        "monthly limit",
        "429",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
        || hits_a_quota_window(&lower)
}

fn hits_a_quota_window(lower: &str) -> bool {
    lower.split("hit your ").skip(1).any(|after| {
        let hours = after.trim_start_matches(|character: char| character.is_ascii_digit());
        if hours.len() < after.len() {
            return hours
                .strip_prefix("-hour")
                .and_then(after_a_whole_limit)
                .is_some();
        }
        let window_end = after.trim_start_matches(|character: char| character.is_ascii_lowercase());
        let Some(after_limit) = after_a_whole_limit(window_end) else {
            return false;
        };
        match after.strip_suffix(window_end) {
            Some("session" | "weekly") => true,
            Some(name) => !name.is_empty() && says_when_it_resets(after_limit),
            None => false,
        }
    })
}

fn after_a_whole_limit(window_end: &str) -> Option<&str> {
    window_end.strip_prefix(" limit").filter(|after_limit| {
        !after_limit
            .starts_with(|character: char| character.is_ascii_alphanumeric() || character == '_')
    })
}

fn says_when_it_resets(after_limit: &str) -> bool {
    after_limit
        .trim_start_matches(|character: char| !(character.is_alphanumeric() || character == '_'))
        .starts_with("resets ")
}

pub fn reset_at_from_limit_message(text: &str, now_unix: u64) -> Option<String> {
    let reset = first_reset(text)?;
    let (date, time) = match leading_month(reset) {
        Some((month, rest)) => {
            let (day, rest) = leading_digits(rest)?;
            let rest = rest.strip_prefix(", ")?;
            (Some((month, day)), rest)
        }
        None => (None, reset),
    };
    let (second_of_day, zone) = time_of_day(time)?;
    if !zone.starts_with(" (UTC)") {
        return None;
    }

    let now = i64::try_from(now_unix).ok()?;
    let today = now.div_euclid(86_400);
    let days = match date {
        None => [today.checked_sub(1)?, today, today.checked_add(1)?].map(Some),
        Some((month, day)) => {
            let (year, _, _) = crate::util::civil_from_days(today);
            [year.checked_sub(1)?, year, year.checked_add(1)?]
                .map(|year| epoch_day(year, month, day))
        }
    };
    let mut nearest: Option<i64> = None;
    for day in days.into_iter().flatten() {
        let at = day.checked_mul(86_400)?.checked_add(second_of_day)?;
        if nearest.is_none_or(|best| at.abs_diff(now) <= best.abs_diff(now)) {
            nearest = Some(at);
        }
    }
    Some(crate::util::rfc3339_utc(u64::try_from(nearest?).ok()?))
}

fn first_reset(text: &str) -> Option<&str> {
    text.match_indices("resets ").find_map(|(at, needle)| {
        let starts_a_word = !text
            .get(..at)?
            .ends_with(|character: char| character.is_ascii_alphanumeric() || character == '_');
        if starts_a_word {
            text.get(at + needle.len()..)
        } else {
            None
        }
    })
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

fn leading_month(text: &str) -> Option<(i64, &str)> {
    MONTHS.iter().zip(1..).find_map(|(name, number)| {
        text.strip_prefix(name)
            .and_then(|rest| rest.strip_prefix(' '))
            .map(|rest| (number, rest))
    })
}

fn leading_digits(text: &str) -> Option<(i64, &str)> {
    let length = text
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(text.len());
    if !(1..=2).contains(&length) {
        return None;
    }
    let (digits, rest) = text.split_at_checked(length)?;
    Some((digits.parse().ok()?, rest))
}

fn time_of_day(text: &str) -> Option<(i64, &str)> {
    let (hour, rest) = leading_digits(text)?;
    let (minute, rest) = match rest.strip_prefix(':') {
        Some(after) => {
            let (minute, rest) = leading_digits(after)?;
            if after.len() - rest.len() != 2 {
                return None;
            }
            (minute, rest)
        }
        None => (0, rest),
    };
    let (afternoon, rest) = match (rest.strip_prefix("am"), rest.strip_prefix("pm")) {
        (Some(rest), _) => (false, rest),
        (None, Some(rest)) => (true, rest),
        (None, None) => return None,
    };
    if !(1..=12).contains(&hour) || !(0..=59).contains(&minute) {
        return None;
    }
    let hour = hour % 12 + if afternoon { 12 } else { 0 };
    Some((hour * 3600 + minute * 60, rest))
}

fn epoch_day(year: i64, month: i64, day: i64) -> Option<i64> {
    let days = crate::util::days_from_civil(year, month, day);
    (crate::util::civil_from_days(days) == (year, month, day)).then_some(days)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_resolves_every_shipped_adapter() {
        assert!(by_id("claude-code").is_some());
        assert!(by_id("copilot").is_some());
        assert!(by_id("codex").is_some());
        assert!(by_id("aider").is_none(), "aider arrives in v0.2");
    }

    fn a_successful_answer_from(id: &str) -> String {
        match id {
            "claude-code" => {
                r#"{"session_id":"s-1","total_cost_usd":0.5,"result":"done","subtype":"success"}"#
                    .to_owned()
            }
            "copilot" => "done".to_owned(),
            "codex" => [
                r#"{"type":"thread.started","thread_id":"th-1"}"#,
                r#"{"type":"item.completed","item":{"type":"agent_message","text":"done"}}"#,
                r#"{"type":"turn.completed","usage":{"input_tokens":11,"output_tokens":7}}"#,
            ]
            .join("\n"),
            other => panic!("an adapter shipped without an answer shape here: {other}"),
        }
    }

    #[test]
    fn every_adapter_maps_every_supervision_result_the_same_way() {
        use crate::ir::OutcomeStatus;

        type SupervisionCell = (
            &'static str,
            Option<i32>,
            bool,
            bool,
            OutcomeStatus,
            &'static str,
        );
        const GRID: &[SupervisionCell] = &[
            (
                "clean success",
                Some(0),
                false,
                false,
                OutcomeStatus::Completed,
                "done",
            ),
            (
                "output-limited although it exited 0",
                Some(0),
                false,
                true,
                OutcomeStatus::AgentError,
                "output limit",
            ),
            (
                "output-limited and terminated",
                None,
                false,
                true,
                OutcomeStatus::AgentError,
                "output limit",
            ),
            (
                "timed out although it exited 0",
                Some(0),
                true,
                false,
                OutcomeStatus::Timeout,
                "wall-clock timeout",
            ),
            (
                "timed out and terminated",
                None,
                true,
                false,
                OutcomeStatus::Timeout,
                "wall-clock timeout",
            ),
            (
                "an ordinary non-zero exit",
                Some(1),
                false,
                false,
                OutcomeStatus::AgentError,
                "",
            ),
        ];

        let mut statuses: Vec<OutcomeStatus> = Vec::new();
        let mut cells = 0_usize;
        for adapter in ADAPTERS {
            let stdout = a_successful_answer_from(adapter.id());
            for (name, code, timed_out, output_limited, expected, must_carry) in GRID {
                let out = ProcessOutput {
                    code: *code,
                    stdout: stdout.clone(),
                    stderr: String::new(),
                    duration: Duration::from_millis(9),
                    timed_out: *timed_out,
                    output_limited: *output_limited,
                };
                let cell = format!("{}/{name}", adapter.id());
                let outcome = adapter
                    .parse(&out)
                    .unwrap_or_else(|error| panic!("{cell}: parse: {error}"));
                assert_eq!(outcome.status, *expected, "{cell}: wrong status");
                if !must_carry.is_empty() {
                    let detail = outcome.detail.clone().unwrap_or_default();
                    assert!(
                        detail.contains(must_carry),
                        "{cell}: the detail must say why (`{must_carry}`): {detail:?}"
                    );
                }
                assert_eq!(outcome.duration, out.duration, "{cell}: duration");
                if !statuses.contains(expected) {
                    statuses.push(*expected);
                }
                cells += 1;
            }
        }

        assert_eq!(GRID.len(), 6, "six supervision shapes");
        assert_eq!(cells, 18, "every shipped adapter crossed with every shape");
        assert_eq!(
            statuses.len(),
            3,
            "Completed, AgentError and Timeout are three distinct answers: {statuses:?}"
        );
        assert_ne!(OutcomeStatus::Timeout, OutcomeStatus::AgentError);
    }

    #[test]
    fn an_agent_probe_request_is_slotted_names_its_agent_and_carries_the_probe_identity() {
        use std::path::Path;

        let request = probe_request(
            "claude-code",
            CommandSpec::new("claude").arg("--version"),
            0,
            Duration::from_secs(60),
        )
        .expect("a shipped adapter id survives an invocation identity");

        assert_eq!(
            request.role,
            ExecutionRole::Probe(ProbeTarget::Agent(AgentId::new("claude-code")))
        );
        assert!(
            request.role.is_slotted(),
            "INV-18: an agent probe is slotted"
        );
        assert_eq!(request.agent, Some(AgentId::new("claude-code")));
        assert_eq!(request.invocation.render(), "p.agent-claude-code.o0");
        assert_eq!(
            request.invocation.probe_target(),
            Some(&ProbeTarget::Agent(AgentId::new("claude-code")))
        );
        assert_eq!(request.command.program, "claude");
        assert_eq!(request.workspace, probe_workspace());
        assert!(
            request.workspace.is_absolute() || request.workspace == Path::new("."),
            "the runner is given an absolute directory unless the cwd is gone"
        );

        assert_ne!(request.invocation.render(), "p.shell.o0");
        assert!(
            !ExecutionRole::Probe(ProbeTarget::Shell).is_slotted(),
            "and the shell probe, which this is not, is the non-slotted one"
        );

        let ids: std::collections::BTreeSet<String> = ADAPTERS
            .iter()
            .map(|adapter| {
                probe_request(
                    adapter.id(),
                    CommandSpec::new("x"),
                    0,
                    Duration::from_secs(1),
                )
                .expect("shipped adapter id")
                .invocation
                .render()
            })
            .collect();
        assert_eq!(ids.len(), ADAPTERS.len());
        assert_eq!(ids.len(), 3, "claude-code, copilot, codex");

        let ordinals: std::collections::BTreeSet<String> = (0..4)
            .map(|ordinal| {
                probe_request(
                    "codex",
                    CommandSpec::new("codex"),
                    ordinal,
                    Duration::from_secs(1),
                )
                .expect("shipped adapter id")
                .invocation
                .render()
            })
            .collect();
        assert_eq!(ordinals.len(), 4);
    }

    #[test]
    fn a_probe_request_refuses_an_agent_id_that_would_not_survive_a_container_name() {
        for id in ["claude.code", "clau de", "", "codex/../etc"] {
            assert!(
                probe_request(id, CommandSpec::new("x"), 0, Duration::from_secs(1)).is_err(),
                "`{id}` must not become an invocation identity"
            );
        }
        for id in ["claude-code", "copilot", "codex", "aider_2"] {
            assert!(
                probe_request(id, CommandSpec::new("x"), 0, Duration::from_secs(1)).is_ok(),
                "`{id}` is a legal agent id"
            );
        }
    }

    #[test]
    fn rate_limit_vocabulary_covers_both_vendors() {
        for phrase in [
            "5-hour limit reached ∙ resets 6pm",
            "Weekly limit reached",
            "API error: rate_limit_error",
            "You are out of credits for this month",
            "premium request allowance exhausted",
            "HTTP 429",
        ] {
            assert!(looks_rate_limited(phrase), "should signal: {phrase}");
        }
        assert!(!looks_rate_limited("wrote the pagination cursor encoder"));
    }

    #[test]
    fn the_observed_hit_your_five_hour_limit_message_signals() {
        assert!(looks_rate_limited(
            "You've hit your 5-hour limit · resets 5:30am (UTC)"
        ));
    }

    #[test]
    fn a_hit_your_weekly_limit_message_signals() {
        assert!(looks_rate_limited("You've hit your weekly limit"));
    }

    #[test]
    fn the_recorded_hit_your_session_limit_message_signals() {
        assert!(looks_rate_limited("You've hit your session limit"));
    }

    #[test]
    fn a_hit_your_context_window_limit_message_does_not_signal() {
        let text = "You've hit your context-window limit; shorten the prompt";
        assert!(!looks_rate_limited(text), "should not signal: {text}");
    }

    #[test]
    fn hit_your_limit_with_no_window_does_not_signal() {
        let text = "you hit your limit";
        assert!(!looks_rate_limited(text), "should not signal: {text}");
    }

    #[test]
    fn an_hours_window_is_spelt_hour_then_limit() {
        let text = "hit your 5-hours limit";
        assert!(!looks_rate_limited(text), "should not signal: {text}");
    }

    #[test]
    fn a_window_after_a_later_hit_your_signals() {
        let text = "the patch hit your branch, then you've hit your weekly limit";
        assert!(looks_rate_limited(text), "should signal: {text}");
    }

    #[test]
    fn limit_before_hit_your_and_none_after_does_not_signal() {
        for text in [
            "no limit was hit; your change is in",
            "lifted the request limit before the patch hit your branch",
        ] {
            assert!(!looks_rate_limited(text), "should not signal: {text}");
        }
    }

    #[test]
    fn hit_your_without_limit_does_not_signal() {
        assert!(!looks_rate_limited("the patch hit your branch cleanly"));
    }

    #[test]
    fn a_session_limiter_does_not_signal() {
        let text = "The request hit your session limiter and returned an internal error.";
        assert!(!looks_rate_limited(text), "should not signal: {text}");
    }

    #[test]
    fn a_weekly_limitation_does_not_signal() {
        let text = "you hit your weekly limitation";
        assert!(!looks_rate_limited(text), "should not signal: {text}");
    }

    #[test]
    fn a_five_hour_limited_quota_does_not_signal() {
        let text = "you've hit your 5-hour limited quota";
        assert!(!looks_rate_limited(text), "should not signal: {text}");
    }

    #[test]
    fn a_full_stop_after_limit_signals() {
        let text = "You've hit your session limit.";
        assert!(looks_rate_limited(text), "should signal: {text}");
    }

    #[test]
    fn limit_at_the_very_end_of_the_text_signals() {
        let text = "You've hit your weekly limit";
        assert!(looks_rate_limited(text), "should signal: {text}");
    }

    #[test]
    fn the_observed_hit_your_fable_limit_message_signals() {
        assert!(looks_rate_limited(FABLE), "should signal: {FABLE}");
    }

    #[test]
    fn a_hit_your_opus_limit_message_with_a_reset_signals() {
        let text = "You've hit your Opus limit · resets 3pm (UTC)";
        assert!(looks_rate_limited(text), "should signal: {text}");
    }

    #[test]
    fn a_hit_your_token_limit_message_without_a_reset_does_not_signal() {
        for text in [
            "You've hit your token limit",
            "You've hit your context limit; shorten the prompt",
        ] {
            assert!(!looks_rate_limited(text), "should not signal: {text}");
        }
    }

    #[test]
    fn a_model_named_limit_without_a_reset_does_not_signal() {
        let text = "You've hit your Fable limit";
        assert!(!looks_rate_limited(text), "should not signal: {text}");
    }

    #[test]
    fn a_named_limit_whose_next_word_is_not_resets_does_not_signal() {
        let text = "You've hit your token limit; the cache resets 9am (UTC)";
        assert!(!looks_rate_limited(text), "should not signal: {text}");
    }

    #[test]
    fn a_named_limiter_with_a_reset_does_not_signal() {
        let text = "You've hit your Fable limiter · resets Nov 14, 9am (UTC)";
        assert!(!looks_rate_limited(text), "should not signal: {text}");
    }

    #[test]
    fn a_name_of_more_than_one_word_with_a_reset_does_not_signal() {
        for text in [
            "You've hit your context window limit · resets 9am (UTC)",
            "You've hit your context-window limit · resets 9am (UTC)",
            "You've hit your Fable4 limit · resets 9am (UTC)",
        ] {
            assert!(!looks_rate_limited(text), "should not signal: {text}");
        }
    }

    const FIVE_HOUR: &str = "You've hit your 5-hour limit · resets 5:30am (UTC)";
    const FABLE: &str = "You've hit your Fable limit · resets Nov 14, 9am (UTC)";
    const HOUR: u64 = 3_600;
    const DAY: u64 = 86_400;
    const NOV_11_2026: u64 = 1_794_355_200;
    const DEC_31_2026: u64 = 1_798_675_200;
    const FEB_27_2026: u64 = 1_772_150_400;
    const FEB_27_2027: u64 = 1_803_686_400;
    const FEB_27_2028: u64 = 1_835_222_400;
    const FEB_27_2029: u64 = 1_866_844_800;

    fn reset(text: &str, now: u64) -> Option<String> {
        reset_at_from_limit_message(text, now)
    }

    #[test]
    fn the_reset_fixtures_are_the_midnights_they_are_named_for() {
        for (now, midnight) in [
            (NOV_11_2026, "2026-11-11T00:00:00Z"),
            (DEC_31_2026, "2026-12-31T00:00:00Z"),
            (FEB_27_2026, "2026-02-27T00:00:00Z"),
            (FEB_27_2027, "2027-02-27T00:00:00Z"),
            (FEB_27_2028, "2028-02-27T00:00:00Z"),
            (FEB_27_2029, "2029-02-27T00:00:00Z"),
        ] {
            assert_eq!(crate::util::rfc3339_utc(now), midnight);
        }
    }

    #[test]
    fn the_observed_five_hour_message_resets_later_the_same_day() {
        assert_eq!(
            reset(FIVE_HOUR, NOV_11_2026 + 3 * HOUR).as_deref(),
            Some("2026-11-11T05:30:00Z")
        );
    }

    #[test]
    fn the_observed_fable_message_resets_on_its_date() {
        assert_eq!(
            reset(FABLE, NOV_11_2026 + DAY + 10 * HOUR).as_deref(),
            Some("2026-11-14T09:00:00Z")
        );
    }

    #[test]
    fn a_time_only_reset_read_just_after_itself_is_its_own() {
        let at_the_reset = NOV_11_2026 + 5 * HOUR + 30 * 60;
        for now in [at_the_reset, at_the_reset + 1, NOV_11_2026 + 6 * HOUR] {
            assert_eq!(
                reset(FIVE_HOUR, now).as_deref(),
                Some("2026-11-11T05:30:00Z"),
                "read at {}",
                crate::util::rfc3339_utc(now)
            );
        }
    }

    #[test]
    fn a_time_only_reset_read_the_evening_before_is_the_next_mornings() {
        assert_eq!(
            reset(FIVE_HOUR, NOV_11_2026 + 20 * HOUR).as_deref(),
            Some("2026-11-12T05:30:00Z")
        );
        let halfway = NOV_11_2026 + 17 * HOUR + 30 * 60;
        assert_eq!(
            reset(FIVE_HOUR, halfway - 1).as_deref(),
            Some("2026-11-11T05:30:00Z"),
            "a second short of halfway, the morning just gone is the nearer"
        );
        assert_eq!(
            reset(FIVE_HOUR, halfway).as_deref(),
            Some("2026-11-12T05:30:00Z"),
            "halfway between two mornings, the later is taken"
        );
    }

    #[test]
    fn twelve_am_is_midnight_and_twelve_pm_is_noon() {
        let now = NOV_11_2026 + HOUR;
        for (text, expected) in [
            ("resets 12am (UTC)", "2026-11-11T00:00:00Z"),
            ("resets 12:45am (UTC)", "2026-11-11T00:45:00Z"),
            ("resets 12pm (UTC)", "2026-11-11T12:00:00Z"),
            ("resets 12:45pm (UTC)", "2026-11-11T12:45:00Z"),
            ("resets 1am (UTC)", "2026-11-11T01:00:00Z"),
            ("resets 1pm (UTC)", "2026-11-11T13:00:00Z"),
            ("resets 11:59pm (UTC)", "2026-11-10T23:59:00Z"),
        ] {
            assert_eq!(reset(text, now).as_deref(), Some(expected), "{text}");
        }
    }

    #[test]
    fn a_dated_reset_is_its_own_until_the_next_years_is_nearer() {
        let the_reset = NOV_11_2026 + 3 * DAY + 9 * HOUR;
        for now in [the_reset + 11 * HOUR, the_reset + DAY, the_reset + DAY + 1] {
            assert_eq!(
                reset(FABLE, now).as_deref(),
                Some("2026-11-14T09:00:00Z"),
                "read at {}",
                crate::util::rfc3339_utc(now)
            );
        }
        let halfway = the_reset + 365 * DAY / 2;
        assert_eq!(crate::util::rfc3339_utc(halfway), "2027-05-15T21:00:00Z");
        assert_eq!(
            reset(FABLE, halfway - 1).as_deref(),
            Some("2026-11-14T09:00:00Z")
        );
        assert_eq!(
            reset(FABLE, halfway).as_deref(),
            Some("2027-11-14T09:00:00Z"),
            "halfway between two years' resets, the later is taken"
        );
    }

    #[test]
    fn a_dated_reset_across_a_year_end_is_next_years() {
        let now = DEC_31_2026 + 20 * HOUR;
        assert_eq!(
            reset("resets Jan 2, 9am (UTC)", now).as_deref(),
            Some("2027-01-02T09:00:00Z")
        );
        assert_eq!(
            reset("resets 5am (UTC)", now).as_deref(),
            Some("2027-01-01T05:00:00Z")
        );
        assert_eq!(
            reset("resets Dec 31, 11pm (UTC)", now).as_deref(),
            Some("2026-12-31T23:00:00Z")
        );
        let dec_30 = DEC_31_2026 - DAY + 12 * HOUR;
        assert_eq!(
            reset("resets Jan 2, 9am (UTC)", dec_30).as_deref(),
            Some("2027-01-02T09:00:00Z")
        );
    }

    #[test]
    fn a_reset_read_just_after_a_year_end_is_last_years() {
        let now = DEC_31_2026 + DAY + 30 * 60;
        assert_eq!(crate::util::rfc3339_utc(now), "2027-01-01T00:30:00Z");
        assert_eq!(
            reset("resets Dec 31, 11pm (UTC)", now).as_deref(),
            Some("2026-12-31T23:00:00Z")
        );
        assert_eq!(
            reset("resets 11pm (UTC)", now).as_deref(),
            Some("2026-12-31T23:00:00Z")
        );
    }

    #[test]
    fn feb_29_is_a_neighbouring_years_leap_day_or_none() {
        let text = "resets Feb 29, 3pm (UTC)";
        for (now, expected, why) in [
            (FEB_27_2026, None, "none of 2025, 2026 and 2027 has the day"),
            (
                FEB_27_2027,
                Some("2028-02-29T15:00:00Z"),
                "2027 has no such day, the next year has",
            ),
            (
                FEB_27_2028,
                Some("2028-02-29T15:00:00Z"),
                "a leap year's own",
            ),
            (
                FEB_27_2029,
                Some("2028-02-29T15:00:00Z"),
                "2029 has no such day, the previous year has, and it is in the past",
            ),
        ] {
            assert_eq!(reset(text, now).as_deref(), expected, "{why}");
        }
    }

    #[test]
    fn a_zone_other_than_utc_gives_none() {
        let now = NOV_11_2026;
        for text in [
            "You've hit your 5-hour limit · resets 5:30am (Europe/London)",
            "resets 5:30am (GMT)",
            "resets 5:30am (utc)",
            "resets 5:30am UTC",
            "resets 5:30am(UTC)",
            "resets 5:30am",
            "resets Nov 14, 9am (America/New_York)",
        ] {
            assert_eq!(reset(text, now), None, "{text}");
        }
    }

    #[test]
    fn text_without_a_reset_gives_none() {
        for text in [
            "",
            "You've hit your session limit",
            "5-hour limit reached",
            "pool rate-limited: 5:30am (UTC)",
            "loaded presets 5:30am (UTC)",
            "reset 5:30am (UTC)",
            "resets",
        ] {
            assert_eq!(reset(text, NOV_11_2026), None, "{text}");
        }
    }

    #[test]
    fn malformed_hours_minutes_months_and_days_give_none() {
        for text in [
            "resets 0am (UTC)",
            "resets 13am (UTC)",
            "resets 13pm (UTC)",
            "resets 123am (UTC)",
            "resets -5am (UTC)",
            "resets +5am (UTC)",
            "resets 5 am (UTC)",
            "resets 5AM (UTC)",
            "resets 5 (UTC)",
            "resets :30am (UTC)",
            "resets 5:60am (UTC)",
            "resets 5:99am (UTC)",
            "resets 5:3am (UTC)",
            "resets 5:300am (UTC)",
            "resets 5:am (UTC)",
            "resets 5.30am (UTC)",
            "resets Noc 14, 9am (UTC)",
            "resets nov 14, 9am (UTC)",
            "resets November 14, 9am (UTC)",
            "resets Sept 14, 9am (UTC)",
            "resets Nov 31, 9am (UTC)",
            "resets Apr 31, 9am (UTC)",
            "resets Feb 30, 9am (UTC)",
            "resets Nov 0, 9am (UTC)",
            "resets Nov 100, 9am (UTC)",
            "resets Nov , 9am (UTC)",
            "resets Nov 14 9am (UTC)",
            "resets Nov 14,9am (UTC)",
            "resets Nov 14, 13pm (UTC)",
            "resets Nov 14, (UTC)",
        ] {
            assert_eq!(reset(text, NOV_11_2026), None, "{text}");
        }
    }

    #[test]
    fn the_first_of_several_resets_is_read() {
        let now = NOV_11_2026;
        assert_eq!(
            reset("resets 5am (UTC), then resets 7am (UTC)", now).as_deref(),
            Some("2026-11-11T05:00:00Z")
        );
        assert_eq!(
            reset("resets Nov 14, 9am (UTC); resets 5am (UTC)", now).as_deref(),
            Some("2026-11-14T09:00:00Z")
        );
        assert_eq!(
            reset("resets 13am (UTC), then resets 7am (UTC)", now),
            None,
            "a malformed first reset is not passed over for a later one"
        );
    }

    #[test]
    fn arbitrary_non_ascii_text_never_panics() {
        let texts = [
            "resets ５am (UTC)",
            "resets 5:3０am (UTC)",
            "resets Növ 14, 9am (UTC)",
            "resets Nov 1４, 9am (UTC)",
            "resets 9ám (UTC)",
            "resets 9am (ＵＴＣ)",
            "resets 9am\u{a0}(UTC)",
            "resets ·",
            "resets é",
            "·resets·5am·(UTC)",
            "制限に達しました · resets 5:30am (UTC)",
            "🙂resets 5am (UTC)",
            "\u{0}resets \u{0}",
        ];
        for now in [0, NOV_11_2026, DEC_31_2026, u64::MAX / 2, u64::MAX] {
            for text in texts.iter().chain([FIVE_HOUR, FABLE].iter()) {
                let _ = reset(text, now);
            }
            for message in [FIVE_HOUR, FABLE] {
                for (index, _) in message.char_indices() {
                    let prefix = message.get(..index).unwrap_or_default();
                    assert_eq!(reset(prefix, now), None, "{prefix:?}");
                    let suffix = message.get(index..).unwrap_or_default();
                    let _ = reset(suffix, now);
                }
            }
        }
        assert_eq!(reset("resets 5:3０am (UTC)", NOV_11_2026), None);
        assert_eq!(
            reset("制限に達しました · resets 5:30am (UTC)", NOV_11_2026).as_deref(),
            Some("2026-11-11T05:30:00Z")
        );
        assert_eq!(reset(FIVE_HOUR, u64::MAX), None);
        assert_eq!(reset(FABLE, u64::MAX), None);
    }

    #[test]
    fn effort_help_is_scoped_to_the_effort_option_and_requires_every_level() {
        let claude = "  --effort <level>  Effort level (low, medium, high, xhigh, max)\n\
                      --model <model>   Model to use\n";
        assert_eq!(missing_effort_levels(claude), []);

        let copilot = "  --effort, --reasoning-effort <level>  Reasoning effort \
                       (choices: \"none\", \"minimal\", \"low\", \"medium\", \"high\", \
                       \"xhigh\", \"max\")\n  --model <model>  Model\n";
        assert_eq!(missing_effort_levels(copilot), []);

        let narrower = "  --effort <level>  Effort level (low, medium, high)\n\
                         --other <value>  xhigh and max appear outside the option\n";
        assert_eq!(
            missing_effort_levels(narrower),
            [Effort::XHigh, Effort::Max],
            "another option cannot supply missing effort choices"
        );
    }

    #[test]
    fn short_flags_are_not_inferred_from_longer_names() {
        assert!(advertises_flag("-p, --print", "-p"));
        assert!(!advertises_flag("--permission-mode", "-p"));
        assert!(!advertises_flag("--settings --share --stdio", "-s"));
        assert!(advertises_flag("-c, --config <key=value>", "--config"));
        assert!(!advertises_flag("--configuration <path>", "--config"));
    }
}

#[cfg(test)]
mod built_program_tests {
    use super::*;
    use crate::ir::{Effort, PermissionMode, WorkerProfile};
    use std::sync::{Mutex, PoisonError};

    struct Boundary {
        installed: String,
        version: String,
        seen: Mutex<Vec<CommandSpec>>,
    }

    impl Boundary {
        fn holding(installed: &str) -> Self {
            Self::holding_version(installed, "9.9.9")
        }

        fn holding_version(installed: &str, version: &str) -> Self {
            Self {
                installed: installed.to_owned(),
                version: version.to_owned(),
                seen: Mutex::new(Vec::new()),
            }
        }

        fn seen(&self) -> Vec<CommandSpec> {
            self.seen
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone()
        }

        fn programs(&self) -> Vec<String> {
            self.seen().into_iter().map(|spec| spec.program).collect()
        }
    }

    impl crate::runner::contract::tests::InlineRunner for Boundary {
        fn run_inline(
            &self,
            request: &RunnerRequest,
        ) -> Result<ProcessOutput, crate::runner::RunnerError> {
            self.seen
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(request.command.clone());
            if request.command.program == self.installed {
                let (code, stdout, stderr) =
                    scripted_answer(&self.installed, &request.command.args, &self.version);
                return Ok(ProcessOutput {
                    code: Some(code),
                    stdout,
                    stderr,
                    duration: Duration::from_millis(1),
                    timed_out: false,
                    output_limited: false,
                });
            }
            Err(crate::runner::RunnerError::never_started(
                &request.invocation,
                UpstrokeError::Agent {
                    message: format!(
                        "`{}` is not present inside this boundary; the agent CLI here is `{}`",
                        request.command.program, self.installed
                    ),
                },
            ))
        }
    }

    fn expected_name(id: &str) -> &'static str {
        match id {
            "claude-code" => "claude",
            "copilot" => "copilot",
            "codex" => "codex",
            other => panic!("an adapter shipped without a name in this table: {other}"),
        }
    }

    fn a_run(agent: &str) -> TaskRun {
        TaskRun {
            prompt: "Do the thing.".to_owned(),
            profile: WorkerProfile {
                name: "impl-mid".to_owned(),
                agent: agent.to_owned(),
                model: "a-model".to_owned(),
                pool: "a-pool".to_owned(),
                permissions: PermissionMode::ReadOnly,
                effort: Some(Effort::Medium),
                max_turns: Some(30),
                extra_args: Vec::new(),
            },
            workspace: PathBuf::from("."),
            gate_cmds: Vec::new(),
            resume_session: None,
            settings_path: None,
        }
    }

    fn scripted_help(cli: &str) -> &'static str {
        match cli {
            "claude" => {
                "  -p, --print\n  --output-format <fmt>\n  --model <model>\n  \
                 --effort <level>  Effort level (low, medium, high, xhigh, max)\n  \
                 --settings <file>\n  --setting-sources <src>\n  --permission-mode <mode>\n  \
                 --resume <id>\n"
            }
            "copilot" => {
                "  -s, --stdin\n  --model <model>\n  \
                 --effort <level>  Effort level (low, medium, high, xhigh, max)\n  \
                 --allow-tool <tool>\n  --deny-tool <tool>\n  --no-ask-user\n"
            }
            "codex" => "--json --sandbox --model -c, --config",
            other => panic!("a CLI shipped without a help script: {other}"),
        }
    }

    fn scripted_answer(cli: &str, args: &[String], version: &str) -> (i32, String, String) {
        let joined = args.join(" ");
        if joined == "--version" {
            return (0, format!("{version}\n"), String::new());
        }
        if joined.ends_with("--help") {
            let help = match (cli, joined.as_str()) {
                ("codex", "exec resume --help") => "--json --model -c, --config",
                _ => scripted_help(cli),
            };
            return (0, help.to_owned(), String::new());
        }
        if joined.contains("upstroke_probe_deliberately_unknown") {
            return (
                2,
                String::new(),
                "error: unknown key `upstroke_probe_deliberately_unknown` in -c override"
                    .to_owned(),
            );
        }
        if joined.contains("model_reasoning_effort=") {
            return (
                2,
                String::new(),
                "error: output schema `upstroke-output-schema-must-not-exist.json` does not exist"
                    .to_owned(),
            );
        }
        if joined == "debug models" {
            let models: Vec<serde_json::Value> = crate::catalog::known_models("codex")
                .into_iter()
                .map(|slug| {
                    serde_json::json!({
                        "slug": slug,
                        "supported_reasoning_levels": Effort::ALL
                            .into_iter()
                            .map(|effort| serde_json::json!({ "effort": effort.to_string() }))
                            .collect::<Vec<_>>(),
                    })
                })
                .collect();
            return (
                0,
                serde_json::json!({ "models": models }).to_string(),
                String::new(),
            );
        }
        (0, String::new(), String::new())
    }

    #[test]
    fn an_adapters_program_is_the_boundarys_and_the_coordinator_host_is_never_asked() {
        let mut host_has = 0_usize;
        let mut host_lacks = 0_usize;
        for adapter in ADAPTERS {
            let name = expected_name(adapter.id());
            let boundary = Boundary::holding(name);

            let spec = adapter
                .build(&a_run(adapter.id()))
                .expect("a named CLI always builds: nothing is resolved");
            assert_eq!(spec.program, name, "{}: built the wrong CLI", adapter.id());
            let program = std::path::Path::new(&spec.program);
            assert!(
                !program.is_absolute(),
                "{}: built `{}`, a location rather than a name",
                adapter.id(),
                spec.program
            );
            assert_eq!(
                program.components().count(),
                1,
                "{}: built `{}`, which carries a directory",
                adapter.id(),
                spec.program
            );

            let caps = adapter
                .probe(&boundary)
                .unwrap_or_else(|error| panic!("{}: {error}", adapter.id()));
            assert_eq!(
                caps.version,
                "9.9.9",
                "{}: certified a version no boundary reported",
                adapter.id()
            );

            let asked = boundary.programs();
            assert!(
                !asked.is_empty(),
                "{}: the boundary was never asked what it has",
                adapter.id()
            );
            assert!(
                asked.iter().all(|program| program == name),
                "{}: a request carried something other than the CLI name: {asked:?}",
                adapter.id()
            );
            assert_eq!(
                boundary.seen()[0].args,
                vec!["--version".to_owned()],
                "{}: the first thing asked of a boundary is not what it has",
                adapter.id()
            );

            match crate::util::find_program(name) {
                Some(resolved) => {
                    assert_ne!(
                        spec.program,
                        resolved.to_string_lossy(),
                        "{}: the program is this host's resolution of the name",
                        adapter.id()
                    );
                    host_has += 1;
                }
                None => host_lacks += 1,
            }
        }
        assert_eq!(
            host_has + host_lacks,
            ADAPTERS.len(),
            "every shipped adapter was measured"
        );
    }

    #[test]
    fn a_cli_this_host_does_not_have_still_certifies_at_the_boundary() {
        let absent: Vec<_> = ADAPTERS
            .iter()
            .filter(|adapter| crate::util::find_program(expected_name(adapter.id())).is_none())
            .collect();
        assert!(
            !absent.is_empty(),
            "every shipped agent CLI is installed on this machine, so \"absent on the host, \
             present at the boundary\" cannot be observed here"
        );

        for adapter in absent {
            let name = expected_name(adapter.id());
            let boundary = Boundary::holding(name);
            let caps = adapter.probe(&boundary).unwrap_or_else(|error| {
                panic!(
                    "{}: a CLI absent from this host was refused before the boundary answered: \
                     {error}",
                    adapter.id()
                )
            });
            assert_eq!(caps.version, "9.9.9");
            assert!(
                adapter.build(&a_run(adapter.id())).is_ok(),
                "{}: a CLI absent from this host could not be built for",
                adapter.id()
            );
        }
    }

    #[test]
    fn a_cli_this_host_does_not_have_is_refused_by_name_and_by_boundary() {
        use crate::runner::host::HostRunner;

        let absent: Vec<_> = ADAPTERS
            .iter()
            .filter(|adapter| crate::util::find_program(expected_name(adapter.id())).is_none())
            .collect();
        assert!(
            !absent.is_empty(),
            "every shipped agent CLI is installed on this machine, so a host-boundary refusal \
             cannot be observed here"
        );

        let runner = HostRunner::new();
        let mut messages = std::collections::BTreeSet::new();
        for adapter in absent {
            let name = expected_name(adapter.id());
            let error = adapter
                .probe(&runner)
                .expect_err("this host has no such CLI, so the boundary cannot certify it");
            let message = error.to_string();
            assert!(
                message.contains(name),
                "{}: the refusal must name the CLI: {message}",
                adapter.id()
            );
            assert!(
                message.contains("installed inside the boundary that executes it"),
                "{}: the refusal must say which boundary is missing it: {message}",
                adapter.id()
            );
            assert!(
                message.contains("on PATH for the host runner"),
                "{}: the refusal must say what to do about it: {message}",
                adapter.id()
            );
            assert!(
                message.contains("no `") && message.contains("` on PATH either"),
                "{}: the refusal must say this host does not have it either: {message}",
                adapter.id()
            );
            assert!(
                message.contains("nothing of that name is on the PATH this runner composes"),
                "{}: the CLI was not refused before the spawn: {message}",
                adapter.id()
            );
            assert!(
                !message.contains("failed to spawn"),
                "{}: the boundary tried to spawn a name it could not resolve: {message}",
                adapter.id()
            );
            messages.insert(message);
        }
        assert_eq!(
            messages.len(),
            ADAPTERS
                .iter()
                .filter(|adapter| crate::util::find_program(expected_name(adapter.id())).is_none())
                .count(),
            "two absent CLIs produced one refusal: {messages:?}"
        );
    }

    #[test]
    fn the_program_preflight_certifies_is_the_program_the_attempt_would_run() {
        for adapter in ADAPTERS {
            let name = expected_name(adapter.id());

            let build_first = Boundary::holding(name);
            let built_first = adapter.build(&a_run(adapter.id())).expect("build");
            adapter.probe(&build_first).expect("probe");

            let probe_first = Boundary::holding(name);
            adapter.probe(&probe_first).expect("probe");
            let built_second = adapter.build(&a_run(adapter.id())).expect("build");

            assert_eq!(
                built_first,
                built_second,
                "{}: the order of build and probe changed what build produces",
                adapter.id()
            );
            assert_eq!(
                build_first.programs(),
                probe_first.programs(),
                "{}: the order changed what pre-flight sent",
                adapter.id()
            );
            for program in build_first.programs() {
                assert_eq!(
                    program,
                    built_first.program,
                    "{}: pre-flight certified `{program}` while the attempt would run `{}`",
                    adapter.id(),
                    built_first.program
                );
            }
        }
    }

    #[test]
    fn two_boundaries_in_one_process_each_certify_their_own_cli() {
        use std::collections::BTreeSet;

        for adapter in ADAPTERS {
            let name = expected_name(adapter.id());
            let mut versions = BTreeSet::new();

            for order in [["1.1.1", "2.2.2"], ["2.2.2", "1.1.1"]] {
                let first = Boundary::holding_version(name, order[0]);
                let second = Boundary::holding_version(name, order[1]);

                let from_first = adapter.probe(&first).expect("first boundary");
                let from_second = adapter.probe(&second).expect("second boundary");

                assert_eq!(
                    from_first.version,
                    order[0],
                    "{}: the first boundary's own version was not what it certified",
                    adapter.id()
                );
                assert_eq!(
                    from_second.version,
                    order[1],
                    "{}: the second boundary in this process was handed the first one's answer",
                    adapter.id()
                );
                assert_eq!(
                    first.programs(),
                    second.programs(),
                    "{}: the two boundaries were asked different things",
                    adapter.id()
                );
                versions.insert(from_first.version);
                versions.insert(from_second.version);
            }
            assert_eq!(
                versions.len(),
                2,
                "{}: the two boundaries did not report two distinct versions, so a shared \
                 answer would be unobservable",
                adapter.id()
            );
        }
    }

    #[test]
    fn the_adapters_hold_no_process_wide_resolution_state() {
        const HOLDERS: [&str; 4] = ["static ", "thread_local!", "OnceLock", "LazyLock"];

        let sources = [
            ("src/agent/bin.rs", include_str!("bin.rs")),
            ("src/agent/claude.rs", include_str!("claude.rs")),
            ("src/agent/codex.rs", include_str!("codex.rs")),
            ("src/agent/copilot.rs", include_str!("copilot.rs")),
        ];

        assert_eq!(sources.len(), ADAPTERS.len() + 1, "each adapter and bin.rs");
        let holders_in = |source: &str| {
            let code = crate::effects::production_code(source);
            HOLDERS
                .into_iter()
                .filter(|holder| {
                    code.match_indices(*holder).any(|(at, _)| {
                        *holder != "static "
                            || code.get(..at).and_then(|before| before.chars().next_back())
                                != Some('\'')
                    })
                })
                .collect::<Vec<_>>()
        };

        let inert = r###"
pub const REVIEW_CANARY_TEXT: &str = "OnceLock";
fn label() -> &'static str { "OnceLock" }
const RAW: &str = r#"static CACHE: LazyLock; #[cfg(test)]"#;
const BYTES: &[u8] = b"thread_local!";
const RAW_BYTES: &[u8] = br#"OnceLock"#;
const CHARACTER: char = 's';
// static COMMENT: OnceLock<LazyLock<()>>;
/* thread_local! { static BLOCK: usize = 0; } */
"###;
        assert!(
            holders_in(inert).is_empty(),
            "literals and comments are inert"
        );
        let test_only = r#"
#[cfg(test)]
mod tests {
    static TEST_CACHE: OnceLock<()> = OnceLock::new();
}
"#;
        assert!(
            holders_in(test_only).is_empty(),
            "test-only state is excluded"
        );
        for (declaration, expected) in [
            ("static CACHE: usize = 0;", vec!["static "]),
            (
                "thread_local! { static CACHE: usize = 0; }",
                vec!["static ", "thread_local!"],
            ),
            ("struct Cache { value: OnceLock<String> }", vec!["OnceLock"]),
            ("struct Cache { value: LazyLock<String> }", vec!["LazyLock"]),
        ] {
            assert_eq!(holders_in(declaration), expected, "{declaration}");
            assert_eq!(
                holders_in(&format!("{inert}\n{test_only}\n{declaration}")),
                expected,
                "production state after inert or test-only text remains visible"
            );
        }

        let mut offenders: Vec<String> = Vec::new();
        for (name, source) in sources {
            for holder in holders_in(source) {
                offenders.push(format!("{name}: {holder}"));
            }
        }
        assert!(
            offenders.is_empty(),
            "an adapter holds a value across calls: {offenders:?}. A resolution remembered in one \
             is handed to the next boundary — `PR4-ADAPTER-RESOLVES-ON-THE-HOST`"
        );
    }

    #[test]
    fn the_host_runner_executes_a_bare_program_name_as_it_executes_the_resolved_path() {
        use crate::runner::host::HostRunner;
        use crate::runner::{InvocationId, gate_request};

        let resolved = crate::util::find_program("git")
            .expect("git is on PATH wherever this repository builds");
        let resolved = resolved
            .to_str()
            .expect("this repository's git lives at a Unicode path")
            .to_owned();
        assert_ne!(resolved, "git", "the two program strings must differ");
        assert!(std::path::Path::new(&resolved).is_absolute());

        let runner = HostRunner::new();
        let run_one = |program: &str, ordinal: u32| {
            let request = gate_request(
                CommandSpec::new(program).arg("--version"),
                PathBuf::from(env!("CARGO_MANIFEST_DIR")),
                Duration::from_secs(60),
                InvocationId::probe(crate::runner::ProbeTarget::Shell, ordinal)
                    .expect("a shell probe identity"),
            );
            runner.run_blocking(&request).expect("git --version runs")
        };

        let named = run_one("git", 0);
        let at_path = run_one(&resolved, 1);

        assert_eq!(named.code, Some(0), "{}", named.stderr);
        assert_eq!(
            named.code, at_path.code,
            "the bare name and the resolved path exited differently"
        );
        assert_eq!(
            named.stdout.trim(),
            at_path.stdout.trim(),
            "the bare name and the resolved path are different programs"
        );
        assert!(
            named.stdout.contains("git version"),
            "this did not run git at all: {:?}",
            named.stdout
        );
    }
}

#[cfg(test)]
mod probe_identity_tests {
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn each_agent_probe_request_names_its_own_agent_in_every_field() {
        use crate::runner::{AgentId, CommandSpec, ExecutionRole, ProbeTarget};
        use std::time::Duration;

        fn spec() -> CommandSpec {
            CommandSpec {
                program: "irrelevant".to_owned(),
                args: Vec::new(),
                env: Vec::new(),
                stdin: Vec::new(),
                settings_file: None,
            }
        }

        const NAMES: [&str; 3] = ["claude-code", "codex", "copilot"];
        for order in [
            NAMES.to_vec(),
            NAMES.iter().rev().copied().collect::<Vec<_>>(),
        ] {
            let mut roles = BTreeSet::new();
            let mut agents = BTreeSet::new();
            let mut identities = BTreeSet::new();
            for (index, name) in order.iter().enumerate() {
                let ordinal = u32::try_from(index).expect("small") + 1;
                let request = super::probe_request(name, spec(), ordinal, Duration::from_secs(30))
                    .expect("build a probe request");
                assert_eq!(
                    request.role,
                    ExecutionRole::Probe(ProbeTarget::Agent(AgentId::new(*name))),
                    "the probe role names another agent"
                );
                assert_eq!(
                    request.agent.as_ref().map(AgentId::as_str),
                    Some(*name),
                    "the request's agent is not the one probed"
                );
                let rendered = request.invocation.render();
                assert!(
                    rendered.contains(name),
                    "the invocation identity does not name {name}: {rendered}"
                );
                roles.insert(request.role.label());
                agents.insert(request.agent.map(|agent| agent.as_str().to_owned()));
                identities.insert(rendered);
            }
            assert_eq!(roles.len(), 3, "{roles:?}");
            assert_eq!(agents.len(), 3, "{agents:?}");
            assert_eq!(identities.len(), 3, "{identities:?}");
        }
    }

    fn use_sites(source: &str) -> Vec<String> {
        let mut kept: Vec<String> = Vec::new();
        let mut skipping_to: Option<String> = None;
        let lines: Vec<&str> = source.lines().collect();
        let mut index = 0;
        while index < lines.len() {
            let line = lines[index];
            if let Some(closing) = &skipping_to {
                if line == closing.as_str() {
                    skipping_to = None;
                }
                index += 1;
                continue;
            }
            let trimmed = line.trim_start();
            let indent = &line[..line.len() - trimmed.len()];
            if trimmed.starts_with("mod probe_ordinal {")
                || (trimmed == "#[cfg(test)]"
                    && lines
                        .get(index + 1)
                        .is_some_and(|next| next.trim_start().starts_with("mod ")))
            {
                skipping_to = Some(format!("{indent}}}"));
                index += 1;
                continue;
            }
            if trimmed.contains("probe_ordinal::")
                && !trimmed.starts_with("//")
                && !trimmed.starts_with("*")
            {
                kept.push(trimmed.to_owned());
            }
            index += 1;
        }
        kept
    }

    #[test]
    fn every_probe_call_site_passes_its_own_ordinal_constant() {
        struct Adapter {
            name: &'static str,
            source: &'static str,
            declared: &'static [&'static str],
            block_parts: &'static [&'static str],
            call_sites: usize,
        }
        let adapters = [
            Adapter {
                name: "claude-code",
                source: include_str!("claude.rs"),
                declared: &["VERSION", "HELP", "AUTH_STATUS"],
                block_parts: &[],
                call_sites: 3,
            },
            Adapter {
                name: "copilot",
                source: include_str!("copilot.rs"),
                declared: &["VERSION", "HELP"],
                block_parts: &[],
                call_sites: 2,
            },
            Adapter {
                name: "codex",
                source: include_str!("codex.rs"),
                declared: &[
                    "VERSION",
                    "EXEC_HELP",
                    "RESUME_HELP",
                    "CONFIG_BASE",
                    "CONFIG_PER_SURFACE",
                    "PROBE_MODELS",
                    "LOGIN_STATUS",
                    "DISCOVER_MODELS",
                ],
                block_parts: &["CONFIG_BASE", "CONFIG_PER_SURFACE"],
                call_sites: 7,
            },
        ];

        let mut total_sites = 0_usize;
        for adapter in adapters {
            let sites = use_sites(adapter.source);
            assert!(
                !sites.is_empty(),
                "{}: no ordinal use site was found, so this test measures nothing",
                adapter.name
            );

            let mut used: BTreeMap<String, usize> = BTreeMap::new();
            for site in &sites {
                let mentioned: Vec<String> = site
                    .match_indices("probe_ordinal::")
                    .map(|(at, _)| {
                        site[at + "probe_ordinal::".len()..]
                            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                            .next()
                            .unwrap_or_default()
                            .to_owned()
                    })
                    .collect();
                for name in &mentioned {
                    assert!(
                        adapter.declared.contains(&name.as_str()),
                        "{}: `{site}` names `{name}`, which the module does not declare",
                        adapter.name
                    );
                    *used.entry(name.clone()).or_default() += 1;
                }
                let bare =
                    mentioned.len() == 1 && *site == format!("probe_ordinal::{},", mentioned[0]);
                assert!(
                    bare || mentioned
                        .iter()
                        .all(|name| adapter.block_parts.contains(&name.as_str())),
                    "{}: `{site}` is neither a bare ordinal argument nor an index into a \
                     documented block — an expression over an ordinal is how two processes \
                     come to share one identity",
                    adapter.name
                );
            }

            let collisions: Vec<(&String, &usize)> =
                used.iter().filter(|(_, count)| **count > 1).collect();
            assert!(
                collisions.is_empty(),
                "{}: {collisions:?} reached the Runner from more than one place, so those \
                 processes share an invocation identity",
                adapter.name
            );

            let declared: BTreeSet<&str> = adapter.declared.iter().copied().collect();
            let actually_used: BTreeSet<&str> = used.keys().map(String::as_str).collect();
            assert_eq!(
                actually_used, declared,
                "{}: the declared ordinals and the ones production uses have diverged",
                adapter.name
            );

            let calls = adapter
                .source
                .lines()
                .filter(|line| {
                    let trimmed = line.trim_start();
                    !trimmed.starts_with("///") && trimmed.contains("probe_request(")
                })
                .count();
            assert_eq!(
                calls, adapter.call_sites,
                "{}: probe call sites moved",
                adapter.name
            );
            total_sites += calls;
        }
        assert_eq!(total_sites, 12, "the adapters' probe call sites moved");
    }
}
