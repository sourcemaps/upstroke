//! Advisory Claude subscription readings. The CLI owns its login and profile;
//! this module never reads credentials or values, only whether its profile and
//! override variables are set. PATH discovery, inherited environment
//! and current working directory use the existing Runner probe contract. The
//! Runner bounds capture, kills timed-out descendants and closes stdin at EOF.
//! Only the reset bearer is blanked. No settings, tools, prompt or shell runs.
//! Wire shape: owner-observed Claude Code 2.1.332 get_usage control response;
//! fixtures are synthetic, not a claim of a live bearer read.

#![forbid(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

use serde::Deserialize;
use serde::de::{DeserializeOwned, IgnoredAny};
use serde_json::Value;

use crate::runner::{CommandSpec, Runner};

use super::reset::{self, ResetReport, Unavailable};
use super::transport::{MAX_BODY_BYTES, PROCESS_TIMEOUT};

/// Selects the CLI's profile when set.
pub const PROFILE_ENV: &str = "CLAUDE_CONFIG_DIR";
/// Overrides whatever profile `PROFILE_ENV` selects when set.
pub const OAUTH_TOKEN_ENV: &str = "CLAUDE_CODE_OAUTH_TOKEN";
const MAX_MODELS: usize = 128;
const INPUT: &str = "{\"type\":\"control_request\",\"request_id\":\"usage-only\",\"request\":{\"subtype\":\"get_usage\",\"skip_behaviors\":true}}\n";

/// One route, with no credential discovery or fallback to the other route.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Auth {
    #[default]
    Cli,
    Bearer,
}

/// A percentage and an automatic reset clock; neither is a reset credit.
#[derive(Debug, PartialEq)]
pub struct Window {
    pub utilization: Option<f64>,
    pub resets_at_unix: Option<u64>,
}

/// Fixed labels prevent account identifiers or arbitrary provider text escaping.
#[derive(Debug, PartialEq)]
pub struct ModelWindow {
    pub label: &'static str,
    pub window: Window,
}

/// Unknown groups do not suppress independent useful readings.
#[derive(Debug, PartialEq)]
pub enum Windows {
    Unavailable,
    Disabled,
    Reported {
        five_hour: Option<Window>,
        seven_day: Option<Window>,
        models: Vec<ModelWindow>,
    },
}

/// Advisory output only: never stored or used as a capacity permit.
/// Grants are absent for native CLI windows; only bearer reads check them.
#[derive(Debug, PartialEq)]
pub enum Report {
    Unavailable(Unavailable),
    Read {
        windows: Windows,
        grants: Option<ResetReport>,
    },
}

/// `present` answers whether a variable is set, never its value. A profile
/// with the overriding token set could report another account, so no CLI starts.
pub(super) fn read_cli(
    runner: &dyn Runner,
    present: &dyn Fn(&str) -> bool,
) -> Result<Value, Unavailable> {
    if present(PROFILE_ENV) && present(OAUTH_TOKEN_ENV) {
        return Err(Unavailable::CliProfileOverridden);
    }
    let mut command = CommandSpec::new("claude");
    for argument in [
        "-p",
        "--input-format",
        "stream-json",
        "--output-format",
        "stream-json",
        "--verbose",
        "--no-session-persistence",
        "--strict-mcp-config",
        "--tools",
        "",
        "--setting-sources",
        "",
    ] {
        command = command.arg(argument);
    }
    let request = crate::agent::probe_request(
        "claude-code",
        command.env(reset::TOKEN_ENV, "").stdin(INPUT),
        0,
        PROCESS_TIMEOUT,
    )
    .map_err(|_| Unavailable::CliNotStarted)?;
    let output = runner
        .run_blocking(&request)
        .map_err(|_| Unavailable::CliNotStarted)?;
    if output.timed_out {
        return Err(Unavailable::CliTimedOut);
    }
    if output.output_limited
        || output.stdout.len() > MAX_BODY_BYTES
        || output.stderr.len() > MAX_BODY_BYTES
    {
        return Err(Unavailable::SubscriptionTooLarge);
    }
    if output.code != Some(0) {
        return Err(Unavailable::CliFailed);
    }
    control_payload(&output.stdout)
}

// What the readers decode, by name: `parse` reads the window qualifier and
// the native `rate_limits`, `parse_windows` the window groups and
// `reset::parse_value` the grant group. Nothing else in a response is decoded.
const AVAILABLE: &str = "rate_limits_available";
const RATE_LIMITS: &str = "rate_limits";
const WINDOW_GROUPS: [&str; 4] = ["five_hour", "seven_day", "model_scoped", "limits"];
const GRANT_GROUP: &str = "cedar_ember";

/// A JSON object's members in order, repeats included, each as its decoded
/// name and its value's raw text; `None` unless `text` is exactly one
/// syntactically valid object, so a positional array is never one. Values
/// are checked for syntax only (`IgnoredAny` computes no number and decodes no
/// string), so a member nobody reads cannot fail the object and each one read
/// is decoded on its own terms. A name that does not decode is no name a
/// reader asks for, so its member is left out.
fn members(text: &str) -> Option<Vec<(String, &str)>> {
    serde_json::from_str::<IgnoredAny>(text).ok()?;
    let mut rest = json_space(text).strip_prefix('{')?;
    let mut members = Vec::new();
    if json_space(rest).starts_with('}') {
        return Some(members);
    }
    loop {
        let (name, after) = next_value(json_space(rest))?;
        let (value, after) = next_value(json_space(json_space(after).strip_prefix(':')?))?;
        if let Ok(name) = serde_json::from_str::<String>(name) {
            members.push((name, value));
        }
        // The whole text is valid, so a member is followed by `,` or `}`.
        match json_space(after).strip_prefix(',') {
            Some(after) => rest = after,
            None => return Some(members),
        }
    }
}

/// The JSON value at the start of `text`, checked for syntax only, and what follows it.
fn next_value(text: &str) -> Option<(&str, &str)> {
    let mut values = serde_json::Deserializer::from_str(text).into_iter::<IgnoredAny>();
    values.next()?.ok()?;
    text.split_at_checked(values.byte_offset())
}

/// JSON's four whitespace characters, never Unicode's wider set.
fn json_space(text: &str) -> &str {
    text.trim_start_matches([' ', '\t', '\n', '\r'])
}

enum Member<'t> {
    Absent,
    Once(&'t str),
    Repeated,
}

fn member<'t>(members: &[(String, &'t str)], name: &str) -> Member<'t> {
    let mut found = members.iter().filter(|(member, _)| member == name);
    match (found.next(), found.next()) {
        (None, _) => Member::Absent,
        (Some((_, text)), None) => Member::Once(text),
        (Some(_), Some(_)) => Member::Repeated,
    }
}

// Value's default map reader keeps the last duplicate key. A reading with two
// competing values is ambiguous, including inside a window or grant, so this
// reader refuses a key repeated at any depth, and a non-finite number.
struct UniqueValue(Value);

impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;

        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = UniqueValue;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("JSON with unique object keys")
            }

            fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::Null))
            }

            fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::Bool(value)))
            }

            fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::from(value)))
            }

            fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::from(value)))
            }

            fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<Self::Value, E> {
                serde_json::Number::from_f64(value)
                    .map(|number| UniqueValue(Value::Number(number)))
                    .ok_or_else(|| E::custom("non-finite number"))
            }

            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::String(value.to_owned())))
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(UniqueValue(value)) = sequence.next_element()? {
                    values.push(value);
                }
                Ok(UniqueValue(Value::Array(values)))
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                let mut values = serde_json::Map::new();
                while let Some(key) = map.next_key::<String>()? {
                    if values.contains_key(&key) {
                        return Err(serde::de::Error::custom("duplicate object key"));
                    }
                    let UniqueValue(value) = map.next_value()?;
                    values.insert(key, value);
                }
                Ok(UniqueValue(Value::Object(values)))
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

/// The bearer body, read group by group. Each response group the readers
/// read (`WINDOW_GROUPS`, `GRANT_GROUP`) and the `AVAILABLE` qualifier is
/// decoded on its own: one whose name repeats, or that holds a repeated key or
/// a value that does not decode (a number beyond `f64`), is left null, so it
/// reads as present but malformed and suppresses no other group. Any other
/// member is metadata: checked for syntax only, never decoded, never a group.
/// `None` for invalid JSON, for a non-object body that does not decode with
/// unique keys, or when something was left null and no group survives.
pub(super) fn bearer_groups(body: &str) -> Option<Value> {
    let Some(members) = members(body) else {
        return serde_json::from_str(body)
            .ok()
            .map(|UniqueValue(value)| value);
    };
    let mut read = serde_json::Map::new();
    let (mut voided, mut survived) = (false, false);
    for name in [AVAILABLE, GRANT_GROUP].into_iter().chain(WINDOW_GROUPS) {
        let value = match member(&members, name) {
            Member::Absent => continue,
            Member::Once(text) => serde_json::from_str(text)
                .ok()
                .map(|UniqueValue(value)| value),
            Member::Repeated => None,
        };
        voided |= value.is_none();
        survived |= value.is_some() && name != AVAILABLE;
        read.insert(name.to_owned(), value.unwrap_or(Value::Null));
    }
    (survived || !voided).then_some(Value::Object(read))
}

// The CLI stream is read by name, one envelope at a time, and only as far as
// this request's answer needs. Every line is one JSON object whose `type` is
// read first, so another event's members of any shape are never interpreted.
// A control response's `response` is a named object whose `request_id` is
// read before anything else: another request's status and result are its own
// and are never decoded. What is read must appear once and decode, or the
// answer is ambiguous and refused.
fn control_payload(stdout: &str) -> Result<Value, Unavailable> {
    if !stdout.ends_with('\n') {
        return Err(Unavailable::CliResponse);
    }
    let mut payload = None;
    for line in stdout.lines() {
        let event = envelope(line)?;
        let kind: String = field(&event, "type")?;
        if kind != "control_response" {
            // Other typed stream objects carry no usage authority. New event
            // types can be ignored without interpreting or printing their data.
            if kind.is_empty() {
                return Err(Unavailable::CliResponse);
            }
            continue;
        }
        let response = envelope(required(&event, "response")?)?;
        if field::<String>(&response, "request_id")? != "usage-only" {
            continue;
        }
        if field::<String>(&response, "subtype")? != "success" || payload.is_some() {
            return Err(Unavailable::CliResponse);
        }
        payload = Some(native_windows(required(&response, "response")?)?);
    }
    payload.ok_or(Unavailable::CliResponse)
}

/// The usage payload as the windows reader reads it: the `AVAILABLE`
/// qualifier and the `WINDOW_GROUPS` of `rate_limits`, each once and decoded
/// whole, or no answer. Native grants and every other member are not read, so
/// they are checked for syntax only and can neither void nor feed a window.
fn native_windows(payload: &str) -> Result<Value, Unavailable> {
    let payload = envelope(payload)?;
    let mut read = serde_json::Map::new();
    if let Some(text) = once(&payload, AVAILABLE)? {
        let UniqueValue(available) = decoded(text)?;
        read.insert(AVAILABLE.to_owned(), available);
    }
    if let Some(text) = once(&payload, RATE_LIMITS)? {
        // A non-object holds no window, whatever else it holds.
        let limits = match members(text) {
            None => Value::Null,
            Some(groups) => {
                let mut windows = serde_json::Map::new();
                for name in WINDOW_GROUPS {
                    if let Some(text) = once(&groups, name)? {
                        let UniqueValue(window) = decoded(text)?;
                        windows.insert(name.to_owned(), window);
                    }
                }
                Value::Object(windows)
            }
        };
        read.insert(RATE_LIMITS.to_owned(), limits);
    }
    Ok(Value::Object(read))
}

/// A named object, or no answer: an array or any other value is no envelope.
fn envelope(text: &str) -> Result<Vec<(String, &str)>, Unavailable> {
    members(text).ok_or(Unavailable::CliResponse)
}

fn once<'t>(members: &[(String, &'t str)], name: &str) -> Result<Option<&'t str>, Unavailable> {
    match member(members, name) {
        Member::Absent => Ok(None),
        Member::Once(text) => Ok(Some(text)),
        Member::Repeated => Err(Unavailable::CliResponse),
    }
}

fn required<'t>(members: &[(String, &'t str)], name: &str) -> Result<&'t str, Unavailable> {
    once(members, name)?.ok_or(Unavailable::CliResponse)
}

fn decoded<T: DeserializeOwned>(text: &str) -> Result<T, Unavailable> {
    serde_json::from_str(text).map_err(|_| Unavailable::CliResponse)
}

fn field<T: DeserializeOwned>(members: &[(String, &str)], name: &str) -> Result<T, Unavailable> {
    decoded(required(members, name)?)
}

fn window(value: &Value, percent_key: &str) -> Option<Window> {
    value.as_object()?;
    Some(Window {
        utilization: value
            .get(percent_key)
            .and_then(Value::as_f64)
            .filter(|percent| percent.is_finite() && (0.0..=100.0).contains(percent)),
        resets_at_unix: value
            .get("resets_at")
            .and_then(Value::as_str)
            .and_then(crate::capacity::usage::unix_from_rfc3339),
    })
}

fn model_label(value: Option<&Value>) -> &'static str {
    match value.and_then(Value::as_str) {
        Some("Sonnet" | "Claude Sonnet" | "sonnet") => "Sonnet",
        Some("Opus" | "Claude Opus" | "opus") => "Opus",
        Some("Haiku" | "Claude Haiku" | "haiku") => "Haiku",
        Some("Fable" | "Claude Fable" | "fable") => "Fable",
        _ => "unknown model",
    }
}

pub(super) fn parse(value: &Value, auth: Auth, now_unix: u64) -> Report {
    let limits = match auth {
        Auth::Cli => value.get(RATE_LIMITS).unwrap_or(&Value::Null),
        Auth::Bearer => value,
    };
    // Native usage is authoritative only for windows. Grants are requested
    // separately, or parsed alongside windows from the single bearer GET.
    let grants = match auth {
        Auth::Cli => None,
        Auth::Bearer => Some(reset::parse_value(limits, now_unix)),
    };
    let available = value.get(AVAILABLE);
    let windows = if available.and_then(Value::as_bool) == Some(false) {
        Windows::Disabled
    } else if (auth == Auth::Cli && available.and_then(Value::as_bool) != Some(true))
        || (available.is_some() && available.and_then(Value::as_bool).is_none())
        || !limits.is_object()
    {
        Windows::Unavailable
    } else {
        parse_windows(limits)
    };
    Report::Read { windows, grants }
}

fn parse_windows(limits: &Value) -> Windows {
    let mut models = Vec::new();
    // The native normalized model list and raw provider list are alternative
    // representations. Prefer a present normalized list, even if malformed.
    let (entries, raw) = match limits.get("model_scoped") {
        Some(entries) => (entries.as_array(), false),
        None => (limits.get("limits").and_then(Value::as_array), true),
    };
    if let Some(entries) = entries {
        if entries.len() <= MAX_MODELS {
            for entry in entries {
                if raw && entry.get("kind").and_then(Value::as_str) != Some("weekly_scoped") {
                    continue;
                }
                if let Some(window) = window(entry, if raw { "percent" } else { "utilization" }) {
                    models.push(ModelWindow {
                        label: model_label(if raw {
                            entry.pointer("/scope/model/display_name")
                        } else {
                            entry.get("display_name")
                        }),
                        window,
                    });
                }
            }
        }
    }
    Windows::Reported {
        five_hour: limits
            .get("five_hour")
            .and_then(|v| window(v, "utilization")),
        seven_day: limits
            .get("seven_day")
            .and_then(|v| window(v, "utilization")),
        models,
    }
}

fn render_window(label: &str, window: Option<&Window>, text: &mut String) {
    let percent = window
        .and_then(|w| w.utilization)
        .map_or_else(|| "unknown".to_owned(), |percent| format!("{percent}%"));
    let reset = window
        .and_then(|w| w.resets_at_unix)
        .map_or_else(|| "unknown".to_owned(), crate::util::rfc3339_utc);
    text.push_str(&format!(
        "  {label}: used: {percent}; automatic reset: {reset}\n"
    ));
}

impl Report {
    /// Only validated numbers, normalized clocks and fixed labels are printed.
    #[must_use]
    pub fn render(&self) -> String {
        let (windows, grants) = match self {
            Self::Read { windows, grants } => (windows, grants),
            Self::Unavailable(reason) => {
                return format!("claude subscription usage: unavailable — {reason}\n");
            }
        };
        let mut text = "claude subscription usage: advisory automatic windows\n".to_owned();
        match windows {
            Windows::Unavailable => text.push_str("  windows: unavailable\n"),
            Windows::Disabled => text.push_str("  window reporting: disabled\n"),
            Windows::Reported {
                five_hour,
                seven_day,
                models,
            } => {
                render_window("five-hour", five_hour.as_ref(), &mut text);
                render_window("seven-day", seven_day.as_ref(), &mut text);
                if models.is_empty() {
                    text.push_str("  model windows: not reported\n");
                }
                for model in models {
                    render_window(model.label, Some(&model.window), &mut text);
                }
            }
        }
        if let Some(grants) = grants {
            text.push_str(&grants.render());
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const NOW: u64 = 1_795_435_200;
    const RESET: &str = "2026-11-23T14:00:00+01:00";

    pub(super) fn success(payload: Value) -> String {
        format!(
            "{}\n",
            json!({"type": "control_response", "response": {
                "request_id": "usage-only", "subtype": "success", "response": payload
            }})
        )
    }

    fn read(limits: &Value, auth: Auth) -> Report {
        match auth {
            Auth::Cli => parse(
                &control_payload(&success(json!({
                    "rate_limits_available": true, "rate_limits": limits
                })))
                .expect("synthetic native payload"),
                auth,
                NOW,
            ),
            Auth::Bearer => parse(limits, auth, NOW),
        }
    }

    #[test]
    fn subscription_windows_are_shared_but_only_bearer_payloads_report_grants() {
        for auth in [Auth::Cli, Auth::Bearer] {
            for cedar in [
                Value::Null,
                json!({"eligible": false, "grants": []}),
                json!({
                    "eligible": true, "grants": [{"type": "full", "resets_left": 2,
                    "ends_at": RESET, "paused": false, "usable_now": true}]
                }),
            ] {
                let limits = json!({
                    "five_hour": {"utilization": 25.5, "resets_at": RESET},
                    "seven_day": {"utilization": 100, "resets_at": RESET},
                    "model_scoped": [{"display_name": "Sonnet", "utilization": 0,
                        "resets_at": RESET}],
                    "cedar_ember": cedar
                });
                let text = read(&limits, auth).render();
                assert!(text.contains("five-hour: used: 25.5%"), "{text}");
                assert!(text.contains("seven-day: used: 100%"), "{text}");
                assert!(text.contains("Sonnet: used: 0%"), "{text}");
                assert!(text.contains("automatic reset: 2026-11-23T13:00:00Z"));
                if auth == Auth::Cli {
                    assert!(!text.contains("reset credits:"));
                    assert!(!text.contains("count:"));
                } else if cedar.is_null() {
                    assert!(text.contains("reset credits: unavailable"));
                    assert!(!text.contains("count: 0"));
                } else if cedar["eligible"] == false {
                    assert!(text.contains("explicitly empty inventory"));
                } else {
                    assert!(text.contains("count: 2; kind: full; use-by: 2026-11-23T13:00:00Z"));
                    assert!(text.contains("availability: usable"));
                }
                let only_grants = read(&json!({"cedar_ember": cedar}), auth).render();
                assert!(only_grants.contains("five-hour: used: unknown"));
                assert!(!only_grants.contains("used: 0%"));
            }
            assert_eq!(
                read(&json!({"five_hour": {"utilization": 9}}), auth)
                    .render()
                    .contains("reset credits: unavailable"),
                auth == Auth::Bearer
            );
        }
    }

    #[test]
    fn malformed_window_fields_stay_unknown_without_hiding_other_groups() {
        for bad in [
            Value::Null,
            json!(-1),
            json!(101),
            json!("25"),
            json!(true),
            json!({}),
        ] {
            let text = read(
                &json!({
                    "five_hour": {"utilization": bad, "resets_at": "2026-02-30T00:00:00Z"},
                    "seven_day": {"utilization": 70, "resets_at": RESET},
                    "model_scoped": [null, {"display_name": "secret-id\u{1b}[2J", "utilization": 5,
                        "resets_at": "secret-token"}],
                    "cedar_ember": {"grants": [null]}
                }),
                Auth::Cli,
            )
            .render();
            assert!(text.contains("five-hour: used: unknown; automatic reset: unknown"));
            assert!(text.contains("seven-day: used: 70%"));
            assert!(text.contains("unknown model: used: 5%"));
            assert!(!text.contains("reset credits:"));
            assert!(!text.contains("secret"));
            assert!(!text.contains('\u{1b}'));
        }
    }

    #[test]
    fn disabled_or_missing_native_reporting_never_reports_grants() {
        for available in [Value::Null, json!(false), json!("true")] {
            let text = parse(
                &json!({"rate_limits_available": available,
                    "rate_limits": {"five_hour": {"utilization": 99},
                        "cedar_ember": {"grants": []}}
                }),
                Auth::Cli,
                NOW,
            )
            .render();
            assert!(!text.contains("reset credits:"));
            assert!(!text.contains("99%"));
            assert!(text.contains(if available == false {
                "disabled"
            } else {
                "unavailable"
            }));
        }
        assert!(
            parse(&json!({}), Auth::Cli, NOW)
                .render()
                .contains("windows: unavailable")
        );
    }

    #[test]
    fn raw_weekly_scoped_windows_are_bounded_and_use_fixed_labels() {
        let limits = json!({"limits": [
            {"kind": "weekly_scoped", "scope": {"model": {"display_name": "Opus"}},
                "percent": 42, "resets_at": RESET},
            {"kind": "unknown", "percent": 95},
            {"kind": "weekly_scoped", "scope": {"model": {"display_name": "account-id"}},
                "percent": 8, "resets_at": RESET}
        ]});
        let text = read(&limits, Auth::Bearer).render();
        assert!(text.contains("Opus: used: 42%"));
        assert!(text.contains("unknown model: used: 8%"));
        assert!(!text.contains("account-id"));
        assert!(!text.contains("95%"));
        let text = read(
            &json!({"model_scoped": vec![json!({"utilization": 8}); MAX_MODELS + 1],
                "five_hour": {"utilization": 4}, "cedar_ember": {"grants": []}
            }),
            Auth::Bearer,
        )
        .render();
        assert!(!text.contains("8%"));
        assert!(text.contains("five-hour: used: 4%"));
        assert!(text.contains("explicitly empty inventory"));
    }

    #[test]
    fn control_stream_ignores_proper_noise_but_refuses_ambiguous_or_malformed_answers() {
        let payload = json!({"rate_limits_available": true, "rate_limits": {}});
        let good = success(payload.clone());
        let wrong = good.replace("usage-only", "another-request");
        let noisy =
            format!("{{\"type\":\"tool_progress\",\"secret\":\"not rendered\"}}\n{wrong}{good}");
        assert_eq!(control_payload(&noisy), Ok(payload));
        for bad in [
            String::new(),
            wrong,
            "not json\n".to_owned(),
            format!("{good}{good}"),
            format!("{good}not json\n"),
            good.trim_end().to_owned(),
            good.replace("success", "error"),
            good.replace(
                "\"request_id\":",
                "\"request_id\":\"other\",\"request_id\":",
            ),
            "{\"type\":\"control_response\"}\n".to_owned(),
            good.replace(
                "\"type\":\"control_response\"",
                "\"type\":\"tool_progress\",\"type\":\"control_response\"",
            ),
            format!("{{\"response\":null,{}", good.trim_start_matches('{')),
            success(Value::Null),
            success(json!([])),
            success(json!({"rate_limits": {"five_hour": {"utilization": 2}}}))
                .replace("\"utilization\":2", "\"utilization\":2,\"utilization\":3"),
        ] {
            assert_eq!(
                control_payload(&bad),
                Err(Unavailable::CliResponse),
                "{bad}"
            );
        }
    }

    /// The group splitter reads names and spacing as JSON does, and decodes
    /// only the groups the readers read: metadata holding a number beyond
    /// `f64`, or under a name that does not decode, is checked for syntax and
    /// dropped. Before, the out-of-range metadata failed the whole body.
    #[test]
    fn bearer_groups_split_bodies_as_json_does_and_never_decode_metadata() {
        let body = " { \"five\\u005fhour\" : { \"utilization\" : 31 } ,\r\n\t\"request_id\" : 1e400 , \
                    \"\\ud800\" : [ ] , \"cedar_ember\" : { \"grants\" : [ ] } } \n";
        let value = bearer_groups(body).expect("both groups survive");
        assert_eq!(
            value,
            json!({"five_hour": {"utilization": 31}, "cedar_ember": {"grants": []}})
        );
        let text = parse(&value, Auth::Bearer, NOW).render();
        assert!(text.contains("five-hour: used: 31%"), "{text}");
        assert!(text.contains("explicitly empty inventory"), "{text}");
        for invalid in [
            "{",
            "{\"five_hour\":{},}",
            "{\"five_hour\" {}}",
            "{} {}",
            "{\"five_hour\":1e}",
            "\u{a0}{\"five_hour\":{}}",
        ] {
            assert_eq!(bearer_groups(invalid), None, "{invalid:?}");
        }
    }
}
