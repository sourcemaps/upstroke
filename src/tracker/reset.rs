//! Claude's advisory voluntary reset inventory, separate from quota clocks
//! and billing arithmetic. Wire facts supplied from the MIT-licensed
//! pingdotgg/t3code@226113fee0682579155702b9e182c5a893aabbcf,
//! apps/server/src/provider/claudeResetCredits.ts; independently implemented.
//! No response strings survive into the result: kinds use a fixed vocabulary
//! and timestamps are parsed and rendered in UTC. Unknown fields stay unknown.

#![forbid(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

use serde_json::Value;

use super::provider::{Credential, Secret, UsageRequest, UsageResponse};
use super::subscription::UniqueValue;
use super::transport::{MAX_BODY_BYTES, TransportError};

/// An operator-provided bearer explicitly authorized by the provider for
/// this integration. Native-app-only subscription credentials are unsuitable.
pub const TOKEN_ENV: &str = "UPSTROKE_CLAUDE_RESET_TOKEN";
const URL: &str = "https://api.anthropic.com/api/oauth/usage?cedar_ember=1&skip_spend=1";
const MAX_GRANTS: usize = 128;

pub(super) fn request(secret: Secret) -> UsageRequest {
    UsageRequest {
        url: URL.to_owned(),
        headers: vec![("anthropic-beta", "oauth-2025-04-20".to_owned())],
        credential: Credential {
            header: "Authorization",
            scheme: "Bearer ",
            secret,
        },
    }
}

/// Fixed display vocabulary; unfamiliar provider types are never printed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Weekly,
    Session,
    /// Resets both the session and weekly windows.
    Full,
    /// Restores Fable usage, taking precedence over the reported type.
    Fable,
    Unknown,
}

fn grant_kind(grant: &Value) -> Kind {
    if grant.get("gives_back_weekly").and_then(Value::as_bool) == Some(true) {
        return Kind::Fable;
    }
    if let Some(kind) = grant.get("type") {
        return match kind.as_str() {
            Some("weekly") => Kind::Weekly,
            Some("session") => Kind::Session,
            Some("full") => Kind::Full,
            _ => Kind::Unknown,
        };
    }
    // Only an absent type permits inference; every cleared window must be known.
    let Some(clears) = grant.get("clears").and_then(Value::as_array) else {
        return Kind::Unknown;
    };
    let mut session = false;
    let mut weekly = false;
    for window in clears {
        match window.as_str() {
            Some("five_hour") => session = true,
            Some("seven_day") => weekly = true,
            _ => return Kind::Unknown,
        }
    }
    match (session, weekly) {
        (true, true) => Kind::Full,
        (true, false) => Kind::Session,
        (false, true) => Kind::Weekly,
        (false, false) => Kind::Unknown,
    }
}

/// A point-in-time assessment, never a capacity permit or a redemption.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    Usable,
    NotUsable,
    Unknown,
}

/// One grant. `None` means missing, null, malformed or unrecognized, never
/// zero, false or unlimited. Expiry and availability are separate evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grant {
    pub resets_left: Option<u64>,
    pub ends_at_unix: Option<u64>,
    pub kind: Kind,
    pub gives_back_weekly: Option<bool>,
    pub paused: Option<bool>,
    pub usable_now: Option<bool>,
    pub availability: Availability,
}

/// An explicitly reported inventory; an empty `grants` is known empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inventory {
    pub observed_at_unix: u64,
    pub eligible: Option<bool>,
    pub grants: Vec<Grant>,
}

/// Reasons contain no untrusted response, process output or credential.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Unavailable {
    #[error(
        "set UPSTROKE_CLAUDE_RESET_TOKEN to a bearer explicitly authorized by the provider for this integration; native-app-only credentials are unsuitable; no request sent"
    )]
    MissingCredential,
    #[error(
        "UPSTROKE_CLAUDE_RESET_TOKEN must be a nonempty printable bearer without whitespace; no request sent"
    )]
    InvalidCredential,
    #[error("curl could not start; install curl and check the runner environment")]
    NotStarted,
    #[error("the reset inventory request timed out; retry later")]
    TimedOut,
    #[error("the reset inventory transport failed; check curl connectivity and retry")]
    Transport,
    #[error("the reset inventory response exceeds the size or grant-count limit")]
    TooLarge,
    #[error(
        "the provider refused the credential; check provider authorization for UPSTROKE_CLAUDE_RESET_TOKEN"
    )]
    CredentialRefused,
    #[error("the provider rate limited the request; retry later")]
    RateLimited,
    #[error("the provider returned an unsuccessful status")]
    Status,
    #[error("the provider did not report a recognized reset inventory")]
    NotReported,
    #[error(
        "CLAUDE_CONFIG_DIR and CLAUDE_CODE_OAUTH_TOKEN are both set, and the token overrides the profile, so the windows could be another account's; unset one; Claude CLI not started"
    )]
    CliProfileOverridden,
    #[error("Claude CLI could not start; check the installation and CLI login")]
    CliNotStarted,
    #[error("Claude CLI usage probe timed out")]
    CliTimedOut,
    #[error("Claude CLI usage probe failed; check the CLI login")]
    CliFailed,
    #[error("Claude CLI did not return one valid usage control response")]
    CliResponse,
    #[error("the subscription response exceeds the output or window-count limit")]
    SubscriptionTooLarge,
}

impl Unavailable {
    pub(super) fn from_transport(error: &TransportError) -> Self {
        match error {
            TransportError::NotStarted(_) => Self::NotStarted,
            TransportError::TimedOut => Self::TimedOut,
            TransportError::TooLarge => Self::TooLarge,
            TransportError::Unsendable(_)
            | TransportError::Failed { .. }
            | TransportError::NoStatus => Self::Transport,
        }
    }
}

/// A successful read or an advisory unavailable result. Neither is persisted
/// or fed into pool estimates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResetReport {
    Inventory(Inventory),
    Unavailable(Unavailable),
}

/// Interpret one bounded response at the supplied clock. Explicit empty
/// inventory is distinct from missing/malformed inventory. Individual unknown
/// grant fields remain unknown; a known disqualifier always defeats usability.
#[must_use]
pub fn parse(response: &UsageResponse, now_unix: u64) -> ResetReport {
    match response_value(response) {
        Ok(value) => parse_value(&value, now_unix),
        Err(reason) => ResetReport::Unavailable(reason),
    }
}

/// Shared status, size and JSON checks for the single fixed bearer response.
/// A key repeated in any object is two competing readings, so the body is
/// not reported. Errors carry only a fixed category, never the response body.
pub(super) fn response_value(response: &UsageResponse) -> Result<Value, Unavailable> {
    let unavailable = match response.status {
        200 => None,
        401 | 403 => Some(Unavailable::CredentialRefused),
        429 => Some(Unavailable::RateLimited),
        _ => Some(Unavailable::Status),
    };
    if let Some(reason) = unavailable {
        return Err(reason);
    }
    if response.body.len() > MAX_BODY_BYTES {
        return Err(Unavailable::TooLarge);
    }
    serde_json::from_str(&response.body)
        .map(|UniqueValue(value)| value)
        .map_err(|_| Unavailable::NotReported)
}

/// The same validated grant representation for reset-only and combined bearer reads.
pub(super) fn parse_value(value: &Value, now_unix: u64) -> ResetReport {
    let Some(cedar) = value.get("cedar_ember").and_then(Value::as_object) else {
        return ResetReport::Unavailable(Unavailable::NotReported);
    };
    let Some(grants) = cedar.get("grants").and_then(Value::as_array) else {
        return ResetReport::Unavailable(Unavailable::NotReported);
    };
    if grants.len() > MAX_GRANTS {
        return ResetReport::Unavailable(Unavailable::TooLarge);
    }
    let eligible = cedar.get("eligible").and_then(Value::as_bool);
    let mut parsed = Vec::with_capacity(grants.len());
    for grant in grants {
        if !grant.is_object() {
            return ResetReport::Unavailable(Unavailable::NotReported);
        }
        let resets_left = grant.get("resets_left").and_then(Value::as_u64);
        let ends_at_unix = grant
            .get("ends_at")
            .and_then(Value::as_str)
            .and_then(crate::capacity::usage::unix_from_rfc3339);
        let kind = grant_kind(grant);
        let paused = grant.get("paused").and_then(Value::as_bool);
        let usable_now = grant.get("usable_now").and_then(Value::as_bool);
        let availability = if eligible == Some(false)
            || resets_left == Some(0)
            || ends_at_unix.is_some_and(|expiry| expiry <= now_unix)
            || paused == Some(true)
            || usable_now == Some(false)
        {
            Availability::NotUsable
        } else if eligible == Some(true)
            && resets_left.is_some_and(|count| count > 0)
            && ends_at_unix.is_some_and(|expiry| expiry > now_unix)
            && paused == Some(false)
            && usable_now == Some(true)
        {
            Availability::Usable
        } else {
            Availability::Unknown
        };
        parsed.push(Grant {
            resets_left,
            ends_at_unix,
            kind,
            gives_back_weekly: grant.get("gives_back_weekly").and_then(Value::as_bool),
            paused,
            usable_now,
            availability,
        });
    }
    ResetReport::Inventory(Inventory {
        observed_at_unix: now_unix,
        eligible,
        grants: parsed,
    })
}

fn known_bool(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "yes",
        Some(false) => "no",
        None => "unknown",
    }
}

impl ResetReport {
    /// Safe terminal text built only from fixed labels and parsed values.
    #[must_use]
    pub fn render(&self) -> String {
        let inventory = match self {
            Self::Unavailable(reason) => {
                return format!("claude reset credits: unavailable — {reason}\n");
            }
            Self::Inventory(inventory) => inventory,
        };
        let mut text = format!(
            "claude reset credits: advisory inventory; eligible: {}; grants: {}\n",
            known_bool(inventory.eligible),
            inventory.grants.len()
        );
        if inventory.grants.is_empty() {
            text.push_str("  explicitly empty inventory\n");
        }
        for grant in &inventory.grants {
            let count = grant
                .resets_left
                .map_or_else(|| "unknown".to_owned(), |count| count.to_string());
            let kind = match grant.kind {
                Kind::Weekly => "weekly",
                Kind::Session => "session",
                Kind::Full => "full",
                Kind::Fable => "fable",
                Kind::Unknown => "unknown",
            };
            let use_by = grant
                .ends_at_unix
                .map_or_else(|| "unknown".to_owned(), crate::util::rfc3339_utc);
            let expiry = match grant.ends_at_unix {
                Some(end) if end <= inventory.observed_at_unix => "expired",
                Some(_) => "not expired",
                None => "unknown",
            };
            let availability = match grant.availability {
                Availability::Usable => "usable",
                Availability::NotUsable => "not usable",
                Availability::Unknown => "unknown",
            };
            text.push_str(&format!(
                "  count: {count}; kind: {kind}; use-by: {use_by}; expiry: {expiry}; \
                 availability: {availability}; gives back weekly: {}; paused: {}\n",
                known_bool(grant.gives_back_weekly),
                known_bool(grant.paused)
            ));
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const NOW: u64 = 1_795_435_200;

    fn read(cedar: Value) -> ResetReport {
        parse(
            &UsageResponse {
                status: 200,
                body: json!({"cedar_ember": cedar}).to_string(),
            },
            NOW,
        )
    }

    fn grant() -> Value {
        json!({
            "resets_left": 2,
            "ends_at": crate::util::rfc3339_utc(NOW + 3600),
            "type": "weekly",
            "gives_back_weekly": true,
            "paused": false,
            "usable_now": true
        })
    }

    fn single(grant: Value, eligible: Value) -> Grant {
        let ResetReport::Inventory(mut inventory) =
            read(json!({"eligible": eligible, "grants": [grant]}))
        else {
            panic!("an inventory was reported");
        };
        inventory.grants.pop().expect("one grant was reported")
    }

    #[test]
    fn counts_kinds_and_use_by_are_separate_from_usability() {
        let parsed = single(grant(), json!(true));
        assert_eq!(parsed.resets_left, Some(2));
        assert_eq!(parsed.ends_at_unix, Some(NOW + 3600));
        assert_eq!(parsed.kind, Kind::Fable);
        assert_eq!(parsed.gives_back_weekly, Some(true));
        assert_eq!(parsed.availability, Availability::Usable);
        let report = read(json!({"eligible": true, "grants": [grant()]}));
        let rendered = report.render();
        for expected in [
            "count: 2",
            "kind: fable",
            "use-by: 2026-11-23T13:00:00Z",
            "expiry: not expired",
            "availability: usable",
        ] {
            assert!(rendered.contains(expected), "{rendered}");
        }
    }

    #[test]
    fn known_grant_kinds_and_precedence_reach_the_cli_text() {
        for (fields, kind, label, gives_back) in [
            (json!({"type": "full"}), Kind::Full, "full", "unknown"),
            (
                json!({"type": "session"}),
                Kind::Session,
                "session",
                "unknown",
            ),
            (json!({"type": "weekly"}), Kind::Weekly, "weekly", "unknown"),
            (
                json!({"type": "weekly", "gives_back_weekly": false}),
                Kind::Weekly,
                "weekly",
                "no",
            ),
            (
                json!({"type": "weekly", "gives_back_weekly": true}),
                Kind::Fable,
                "fable",
                "yes",
            ),
            (
                json!({"type": "session", "gives_back_weekly": true}),
                Kind::Fable,
                "fable",
                "yes",
            ),
            (
                json!({"type": "full", "gives_back_weekly": true}),
                Kind::Fable,
                "fable",
                "yes",
            ),
            (
                json!({"type": "future", "gives_back_weekly": true}),
                Kind::Fable,
                "fable",
                "yes",
            ),
            (
                json!({"type": null, "clears": [null], "gives_back_weekly": true}),
                Kind::Fable,
                "fable",
                "yes",
            ),
            (
                json!({"gives_back_weekly": true}),
                Kind::Fable,
                "fable",
                "yes",
            ),
            (
                json!({"clears": ["five_hour"]}),
                Kind::Session,
                "session",
                "unknown",
            ),
            (
                json!({"clears": ["seven_day"]}),
                Kind::Weekly,
                "weekly",
                "unknown",
            ),
            (
                json!({"clears": ["five_hour", "seven_day"]}),
                Kind::Full,
                "full",
                "unknown",
            ),
            (
                json!({"clears": ["seven_day", "five_hour", "seven_day"]}),
                Kind::Full,
                "full",
                "unknown",
            ),
            (
                json!({"type": "session", "clears": ["seven_day", null]}),
                Kind::Session,
                "session",
                "unknown",
            ),
            (
                json!({"type": "future"}),
                Kind::Unknown,
                "unknown",
                "unknown",
            ),
        ] {
            let mut wire = grant();
            let object = wire.as_object_mut().expect("a grant object");
            object.remove("type");
            object.remove("gives_back_weekly");
            // Keep the input fields as a diagnostic snapshot for each assertion.
            object.extend(
                fields
                    .as_object()
                    .expect("classification fields")
                    .to_owned(),
            );
            let report = read(json!({"eligible": true, "grants": [&wire]}));
            assert_eq!(single(wire, json!(true)).kind, kind, "{fields}");
            // This is the complete text printed by capacity --reset-credits claude.
            assert_eq!(
                report.render(),
                format!(
                    "claude reset credits: advisory inventory; eligible: yes; grants: 1\n\
                     \x20 count: 2; kind: {label}; use-by: 2026-11-23T13:00:00Z; \
                     expiry: not expired; availability: usable; \
                     gives back weekly: {gives_back}; paused: no\n"
                ),
                "{fields}"
            );
        }
    }

    #[test]
    fn untrusted_classification_fields_never_guess_or_leak_into_output() {
        let hostile = "secret-kind-\u{1b}[2J\nforged output";
        let mut cases = vec![json!({})];
        for value in [
            Value::Null,
            json!(false),
            json!(5),
            json!({}),
            json!([]),
            json!(""),
            json!("full "),
            json!("fable"),
            json!("future"),
            json!(hostile),
        ] {
            cases.push(json!({"type": value, "clears": ["five_hour", "seven_day"]}));
        }
        for clears in [
            Value::Null,
            json!("five_hour"),
            json!({"five_hour": true}),
            json!([]),
            json!(["unknown"]),
            json!(["five_hour", "seven_day", "unknown"]),
            json!(["seven_day", null]),
            json!([false, "five_hour"]),
            json!(["five_hour", 7]),
            json!(["seven_day", {}]),
            json!(["five_hour", ["seven_day"]]),
            json!(["five_hour", hostile]),
        ] {
            cases.push(json!({"clears": clears}));
        }
        for value in [Value::Null, json!("true"), json!(1), json!({}), json!([])] {
            cases.push(json!({"gives_back_weekly": value}));
        }
        for wire in cases {
            let report = read(json!({"eligible": true, "grants": [&wire]}));
            assert_eq!(single(wire, json!(true)).kind, Kind::Unknown);
            assert_eq!(
                report.render(),
                "claude reset credits: advisory inventory; eligible: yes; grants: 1\n\
                 \x20 count: unknown; kind: unknown; use-by: unknown; expiry: unknown; \
                 availability: unknown; gives back weekly: unknown; paused: unknown\n"
            );
        }
    }

    #[test]
    fn known_disqualifiers_defeat_a_claim_of_usability() {
        for (field, value) in [
            ("resets_left", json!(0)),
            ("paused", json!(true)),
            ("usable_now", json!(false)),
            ("ends_at", json!(crate::util::rfc3339_utc(NOW))),
            ("ends_at", json!(crate::util::rfc3339_utc(NOW - 1))),
        ] {
            let mut wire = grant();
            wire[field] = value;
            let rendered = read(json!({"eligible": true, "grants": [&wire]})).render();
            assert!(rendered.contains("availability: not usable"), "{rendered}");
            assert!(!rendered.contains("availability: usable"), "{rendered}");
            assert!(!rendered.contains("usable now: yes"), "{rendered}");
            assert_eq!(
                single(wire, json!(true)).availability,
                Availability::NotUsable,
                "{field}"
            );
        }
        assert_eq!(
            single(grant(), json!(false)).availability,
            Availability::NotUsable
        );
    }

    #[test]
    fn unknown_fields_never_become_zero_or_usable() {
        for field in ["resets_left", "ends_at", "paused", "usable_now"] {
            for bad in [Value::Null, json!("unrecognized"), json!({}), json!([])] {
                let mut wire = grant();
                wire[field] = bad;
                assert_eq!(
                    single(wire, json!(true)).availability,
                    Availability::Unknown,
                    "{field}"
                );
            }
            let mut wire = grant();
            wire.as_object_mut().expect("a grant object").remove(field);
            assert_eq!(
                single(wire, json!(true)).availability,
                Availability::Unknown
            );
        }
        for count in [json!(-1), json!(1.5), json!(true), json!("0")] {
            let mut wire = grant();
            wire["resets_left"] = count;
            let parsed = single(wire, json!(true));
            assert_eq!(parsed.resets_left, None);
            assert_eq!(parsed.availability, Availability::Unknown);
        }
        for eligible in [Value::Null, json!("true"), json!({})] {
            assert_eq!(
                single(grant(), eligible).availability,
                Availability::Unknown
            );
        }
    }

    #[test]
    fn empty_inventory_is_distinct_from_non_reporting_and_failures() {
        let empty = read(json!({"grants": [], "eligible": false}));
        assert!(empty.render().contains("explicitly empty inventory"));
        for body in [
            "{}",
            "null",
            "not json",
            r#"{"cedar_ember":null}"#,
            r#"{"cedar_ember":{}}"#,
            r#"{"cedar_ember":{"grants":null}}"#,
            r#"{"cedar_ember":{"grants":{}}}"#,
            r#"{"cedar_ember":{"grants":[null]}}"#,
        ] {
            assert_eq!(
                parse(
                    &UsageResponse {
                        status: 200,
                        body: body.to_owned()
                    },
                    NOW
                ),
                ResetReport::Unavailable(Unavailable::NotReported),
                "{body}"
            );
        }
        for (status, reason) in [
            (0, Unavailable::Status),
            (204, Unavailable::Status),
            (302, Unavailable::Status),
            (401, Unavailable::CredentialRefused),
            (403, Unavailable::CredentialRefused),
            (404, Unavailable::Status),
            (429, Unavailable::RateLimited),
            (500, Unavailable::Status),
        ] {
            let report = parse(
                &UsageResponse {
                    status,
                    body: r#"{"cedar_ember":{"eligible":true,"grants":[]}}"#.to_owned(),
                },
                NOW,
            );
            assert_eq!(report, ResetReport::Unavailable(reason));
        }
    }

    #[test]
    fn hostile_strings_are_never_rendered_and_timestamps_are_normalized() {
        let hostile = "secret-id-\u{1b}[2J\nforged output";
        let report = read(json!({"eligible": true, "grants": [{
            "type": hostile, "id": hostile, "ends_at": hostile,
            "resets_left": hostile, "paused": hostile, "usable_now": hostile,
            "gives_back_weekly": hostile
        }]}));
        let text = report.render();
        assert!(!text.contains("secret-id"));
        assert!(!text.contains('\u{1b}'));
        assert!(text.contains("count: unknown; kind: unknown; use-by: unknown"));
        assert!(text.contains("availability: unknown"));
        let mut wire = grant();
        wire["ends_at"] = json!("2026-11-23T14:00:00.123456+01:00");
        assert_eq!(single(wire, json!(true)).ends_at_unix, Some(NOW + 3600));
        let mut wire = grant();
        wire["ends_at"] = json!("2026-02-30T00:00:00Z");
        assert_eq!(single(wire, json!(true)).ends_at_unix, None);
    }

    #[test]
    fn bearer_parser_rejects_contradictory_counts() {
        let ends_at = crate::util::rfc3339_utc(NOW + 3600);
        let body = |counts: &str| {
            format!(
                r#"{{"cedar_ember":{{"eligible":true,"grants":[{{{counts},"ends_at":"{ends_at}","type":"weekly","paused":false,"usable_now":true}}]}}}}"#
            )
        };
        let response = |body: String| UsageResponse { status: 200, body };
        let single = parse(&response(body(r#""resets_left":1"#)), NOW);
        assert!(single.render().contains("count: 1;"), "{single:?}");
        assert!(
            single.render().contains("availability: usable;"),
            "{single:?}"
        );
        let contradictory = response(body(r#""resets_left":0,"resets_left":1"#));
        let report = parse(&contradictory, NOW);
        assert_eq!(
            report,
            ResetReport::Unavailable(Unavailable::NotReported),
            "{}",
            report.render()
        );
        let text = report.render();
        assert!(!text.contains("count: 1"), "{text}");
        assert!(!text.contains("availability: usable"), "{text}");
        assert_eq!(
            response_value(&contradictory),
            Err(Unavailable::NotReported),
            "the combined bearer subscription read shares this reader"
        );
    }

    #[test]
    fn inventory_work_is_bounded_even_with_a_fake_transport() {
        let response = UsageResponse {
            status: 200,
            body: " ".repeat(MAX_BODY_BYTES + 1),
        };
        assert_eq!(
            parse(&response, NOW),
            ResetReport::Unavailable(Unavailable::TooLarge)
        );
        assert_eq!(
            read(json!({"grants": vec![json!({}); MAX_GRANTS + 1]})),
            ResetReport::Unavailable(Unavailable::TooLarge)
        );
    }
}
