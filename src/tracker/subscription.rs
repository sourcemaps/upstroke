//! Advisory Claude subscription readings. The CLI owns its login and profile;
//! this module never reads credentials. PATH discovery, inherited environment
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
use serde_json::Value;

use crate::runner::{CommandSpec, Runner};

use super::reset::{self, ResetReport, Unavailable};
use super::transport::{MAX_BODY_BYTES, PROCESS_TIMEOUT};

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
#[derive(Debug, PartialEq)]
pub enum Report {
    Unavailable(Unavailable),
    Read {
        windows: Windows,
        grants: ResetReport,
    },
}

pub(super) fn read_cli(runner: &dyn Runner) -> Result<Value, Unavailable> {
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

// Typed envelopes reject duplicate routing/status fields as ambiguous.
#[derive(Deserialize)]
struct StreamEvent {
    #[serde(rename = "type")]
    kind: String,
    response: Option<ControlResponse>,
}

#[derive(Deserialize)]
struct ControlResponse {
    request_id: String,
    subtype: String,
    response: Option<UniqueValue>,
}

// Value's default map reader keeps the last duplicate key. A usage payload
// with two competing readings is ambiguous, including inside a window/grant.
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

fn control_payload(stdout: &str) -> Result<Value, Unavailable> {
    if !stdout.ends_with('\n') {
        return Err(Unavailable::CliResponse);
    }
    let mut payload = None;
    for line in stdout.lines() {
        let event: StreamEvent =
            serde_json::from_str(line).map_err(|_| Unavailable::CliResponse)?;
        if event.kind != "control_response" {
            // Other typed stream objects carry no usage authority. New event
            // types can be ignored without interpreting or printing their data.
            if event.kind.is_empty() {
                return Err(Unavailable::CliResponse);
            }
            continue;
        }
        let response = event.response.ok_or(Unavailable::CliResponse)?;
        if response.request_id != "usage-only" {
            continue;
        }
        if response.subtype != "success" || payload.is_some() {
            return Err(Unavailable::CliResponse);
        }
        let UniqueValue(value) = response.response.ok_or(Unavailable::CliResponse)?;
        if !value.is_object() {
            return Err(Unavailable::CliResponse);
        }
        payload = Some(value);
    }
    payload.ok_or(Unavailable::CliResponse)
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
        Auth::Cli => value.get("rate_limits").unwrap_or(&Value::Null),
        Auth::Bearer => value,
    };
    let grants = reset::parse_value(limits, now_unix);
    let available = value.get("rate_limits_available");
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
        text.push_str(&grants.render());
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
    fn windows_and_grants_are_independent_for_both_wire_routes() {
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
                if cedar.is_null() {
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
            assert!(
                read(&json!({"five_hour": {"utilization": 9}}), auth)
                    .render()
                    .contains("reset credits: unavailable")
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
            assert!(text.contains("reset credits: unavailable"));
            assert!(!text.contains("secret"));
            assert!(!text.contains('\u{1b}'));
        }
    }

    #[test]
    fn disabled_or_missing_native_reporting_still_reads_actual_grants() {
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
            assert!(text.contains("explicitly empty inventory"));
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
}
