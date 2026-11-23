//! The provider usage report: the one record the capacity tracker writes and
//! the capacity engine reads (DESIGN.md §13, §15).
//!
//! A report is what one provider usage endpoint said about one pool at one
//! moment: per quota window, how much was drawn and when the window resets.
//! The tracker (`src/tracker.rs`) is the only writer; it asks the endpoint
//! over HTTPS, which the engine never does (§4.2). Everything here is the
//! other side of that line: a persisted schema, its codec, and the read. No
//! function in this module starts a process or opens a socket.
//!
//! The file is on-disk data and so untrusted (CODING_STANDARDS.md §8): it may
//! have been written by a newer build, cut short, or edited by hand. `decode`
//! refuses anything it cannot hold exactly rather than repairing it, and a
//! refused report is no report: the estimate falls back to what the other
//! sources say, never to a guess.

#![forbid(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The schema this build writes and the only one it reads.
pub const SCHEMA: u32 = 1;

/// A report holds a handful of windows; a file claiming more is not one of
/// ours.
pub const MAX_WINDOWS: usize = 16;

/// A window label is a short name (`5h`, `weekly`, `month`), bounded so a
/// report cannot grow a terminal line without limit.
pub const MAX_LABEL_CHARS: usize = 64;

/// A pool or provider name, bounded for the same reason.
pub const MAX_NAME_CHARS: usize = 256;

/// The largest report file `read` will open. A report is a few hundred bytes.
pub const MAX_REPORT_BYTES: u64 = 64 * 1024;

/// The directory under the machine directory that holds the reports,
/// `~/.upstroke/capacity/usage/` by default.
#[must_use]
pub fn store_dir(machine_dir: &Path) -> PathBuf {
    machine_dir.join("capacity").join("usage")
}

/// One quota window of a report.
#[derive(Debug, Clone, PartialEq)]
pub struct UsageWindow {
    /// The provider's window, named for the operator: `month`.
    pub label: String,
    /// The amount the provider says is used, in the pool's own units
    /// (dollars, credits, requests). A billing endpoint reports what was
    /// drawn and not what is left: the allowance it is measured against is
    /// the pool's `monthly_allowance`, which only the reader knows.
    pub used: f64,
    /// When the window resets, in Unix seconds, where that is known.
    pub resets_at: Option<u64>,
}

/// What one provider endpoint reported for one pool.
#[derive(Debug, Clone, PartialEq)]
pub struct UsageReport {
    /// The pool the report is for, exactly as `pools.toml` names it.
    pub pool: String,
    /// The provider endpoint that answered (`usage_provider`).
    pub provider: String,
    /// When the tracker received the answer, in Unix seconds.
    pub fetched_at: u64,
    pub windows: Vec<UsageWindow>,
}

/// Why a report file is not a report.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReportDefect {
    #[error("it is not a usage report this build can parse: {0}")]
    Malformed(String),
    #[error("it is written in schema {found} and this build reads schema {SCHEMA}")]
    Schema { found: u32 },
    #[error("its `{field}` is {problem}")]
    Field {
        field: &'static str,
        problem: String,
    },
    #[error("window {index} {problem}")]
    Window { index: usize, problem: String },
    #[error("it is for pool `{found}`, not the pool this file is named for")]
    OtherPool { found: String },
    #[error("it is {len} bytes, more than the {MAX_REPORT_BYTES} a usage report can be")]
    TooLarge { len: u64 },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawReport {
    schema: u32,
    pool: String,
    provider: String,
    fetched_at: String,
    windows: Vec<RawWindow>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWindow {
    label: String,
    used_amount: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    resets_at: Option<String>,
}

/// Only the `schema` field, read first so that a report from a newer build is
/// refused as that rather than as a parse error naming whichever field the
/// newer schema added.
#[derive(Debug, Deserialize)]
struct SchemaProbe {
    schema: u32,
}

impl UsageReport {
    /// The report as the bytes `decode` reads back: pretty JSON with a
    /// trailing newline, timestamps in RFC 3339 UTC like the event log's.
    ///
    /// # Errors
    ///
    /// [`ReportDefect`] for a report outside the bounds [`validate`] holds,
    /// so that nothing is ever written that the reader would refuse.
    ///
    /// [`validate`]: Self::validate
    pub fn encode(&self) -> Result<String, ReportDefect> {
        self.validate()?;
        let raw = RawReport {
            schema: SCHEMA,
            pool: self.pool.clone(),
            provider: self.provider.clone(),
            fetched_at: crate::util::rfc3339_utc(self.fetched_at),
            windows: self
                .windows
                .iter()
                .map(|window| RawWindow {
                    label: window.label.clone(),
                    used_amount: window.used,
                    resets_at: window.resets_at.map(crate::util::rfc3339_utc),
                })
                .collect(),
        };
        let mut text = serde_json::to_string_pretty(&raw)
            .map_err(|error| ReportDefect::Malformed(error.to_string()))?;
        text.push('\n');
        Ok(text)
    }

    /// Parse and validate a report file's bytes.
    ///
    /// # Errors
    ///
    /// [`ReportDefect`] for anything that is not exactly a schema-1 report
    /// within the bounds above: an unknown field, a missing one, a number out
    /// of range, a timestamp this build cannot read.
    pub fn decode(bytes: &[u8]) -> Result<Self, ReportDefect> {
        let probe: SchemaProbe = serde_json::from_slice(bytes)
            .map_err(|error| ReportDefect::Malformed(error.to_string()))?;
        if probe.schema != SCHEMA {
            return Err(ReportDefect::Schema {
                found: probe.schema,
            });
        }
        let raw: RawReport = serde_json::from_slice(bytes)
            .map_err(|error| ReportDefect::Malformed(error.to_string()))?;
        let fetched_at = unix_from_rfc3339(&raw.fetched_at).ok_or_else(|| ReportDefect::Field {
            field: "fetched_at",
            problem: "not an RFC 3339 timestamp".to_owned(),
        })?;
        let mut windows = Vec::with_capacity(raw.windows.len().min(MAX_WINDOWS));
        for (position, window) in raw.windows.into_iter().enumerate() {
            let index = position + 1;
            let resets_at = match window.resets_at {
                None => None,
                Some(text) => {
                    Some(
                        unix_from_rfc3339(&text).ok_or_else(|| ReportDefect::Window {
                            index,
                            problem: "has a `resets_at` that is not an RFC 3339 timestamp"
                                .to_owned(),
                        })?,
                    )
                }
            };
            windows.push(UsageWindow {
                label: window.label,
                used: window.used_amount,
                resets_at,
            });
        }
        let report = Self {
            pool: raw.pool,
            provider: raw.provider,
            fetched_at,
            windows,
        };
        report.validate()?;
        Ok(report)
    }

    /// The bounds every report holds, checked before one is encoded and after
    /// one is decoded, so the tracker cannot publish what the reader would
    /// refuse.
    ///
    /// # Errors
    ///
    /// [`ReportDefect::Field`] or [`ReportDefect::Window`] naming what is out
    /// of bounds.
    pub fn validate(&self) -> Result<(), ReportDefect> {
        check_name("pool", &self.pool)?;
        check_name("provider", &self.provider)?;
        if self.windows.len() > MAX_WINDOWS {
            return Err(ReportDefect::Field {
                field: "windows",
                problem: format!(
                    "{} entries long, more than the {MAX_WINDOWS} a report holds",
                    self.windows.len()
                ),
            });
        }
        for (position, window) in self.windows.iter().enumerate() {
            let index = position + 1;
            let refuse = |problem: String| ReportDefect::Window { index, problem };
            if window.label.trim().is_empty()
                || window.label.chars().count() > MAX_LABEL_CHARS
                || window.label.chars().any(char::is_control)
            {
                return Err(refuse(format!(
                    "has a label that is empty, longer than {MAX_LABEL_CHARS} characters, or \
                     holds a control character"
                )));
            }
            if !window.used.is_finite() || window.used < 0.0 {
                return Err(refuse(format!(
                    "has `used_amount = {}`, which is not a finite amount of zero or more",
                    window.used
                )));
            }
        }
        Ok(())
    }
}

fn check_name(field: &'static str, value: &str) -> Result<(), ReportDefect> {
    if value.trim().is_empty()
        || value.chars().count() > MAX_NAME_CHARS
        || value.chars().any(char::is_control)
    {
        return Err(ReportDefect::Field {
            field,
            problem: format!(
                "empty, longer than {MAX_NAME_CHARS} characters, or holds a control character"
            ),
        });
    }
    Ok(())
}

/// Where the report for `pool` lives under `store`.
///
/// A pool name is any TOML key, so it is not a file name: the readable part is
/// `filename_component`'s reduction, and the eight hex digits after it are the
/// start of the name's SHA-256, which keeps `a/b` and `a-b` in two files. The
/// file also records the exact name, and `read` checks it.
#[must_use]
pub fn report_path(store: &Path, pool: &str) -> PathBuf {
    let digest = format!("{:x}", Sha256::digest(pool.as_bytes()));
    let tag: String = digest.chars().take(8).collect();
    store.join(format!(
        "{}-{tag}.json",
        crate::util::filename_component(pool)
    ))
}

/// Why a stored report could not be read as one.
#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error("could not read {}: {source}", .path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{} is not a usable report: {defect}", .path.display())]
    Defect { path: PathBuf, defect: ReportDefect },
}

/// The stored report for `pool`, or `None` when the tracker has written none.
///
/// # Errors
///
/// [`ReadError`] when a file is there and is not a report for this pool. Only
/// an actual not-found is absence; a permission error, a directory in the
/// file's place and a malformed file all stay errors, so a caller can say the
/// report was unreadable rather than that the tracker never ran.
pub fn read(store: &Path, pool: &str) -> Result<Option<UsageReport>, ReadError> {
    let path = report_path(store, pool);
    let io_error = |source| ReadError::Io {
        path: path.clone(),
        source,
    };
    let len = match std::fs::metadata(&path) {
        Ok(metadata) => metadata.len(),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(io_error(source)),
    };
    let refuse = |defect| ReadError::Defect {
        path: path.clone(),
        defect,
    };
    if len > MAX_REPORT_BYTES {
        return Err(refuse(ReportDefect::TooLarge { len }));
    }
    let bytes = match crate::util::read_file_bounded(&path) {
        Ok(bytes) => bytes,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(io_error(source)),
    };
    let report = UsageReport::decode(&bytes).map_err(refuse)?;
    if report.pool != pool {
        return Err(refuse(ReportDefect::OtherPool { found: report.pool }));
    }
    Ok(Some(report))
}

/// Unix seconds of an RFC 3339 timestamp, or `None` when `text` is not one.
///
/// Reads what the event log writes (`2026-11-23T12:03:00Z`) and what the
/// provider endpoints write (`2026-11-23T12:03:00.943648+00:00`): a `T` or
/// `t`, an optional fraction, which is dropped, and `Z`, `z` or a numeric
/// offset. A date that does not exist, a field out of range and a moment
/// before 1970 are all `None`; nothing is clamped into range.
#[must_use]
pub fn unix_from_rfc3339(text: &str) -> Option<u64> {
    let bytes = text.as_bytes();
    let digits = |from: usize, len: usize| -> Option<i64> {
        let field = bytes.get(from..from.checked_add(len)?)?;
        field.iter().try_fold(0_i64, |value, byte| {
            byte.is_ascii_digit()
                .then(|| value * 10 + i64::from(byte - b'0'))
        })
    };
    let is = |at: usize, wanted: &[u8]| bytes.get(at).is_some_and(|byte| wanted.contains(byte));

    let year = digits(0, 4)?;
    let month = digits(5, 2)?;
    let day = digits(8, 2)?;
    let hour = digits(11, 2)?;
    let minute = digits(14, 2)?;
    let second = digits(17, 2)?;
    if !(is(4, b"-") && is(7, b"-") && is(10, b"Tt") && is(13, b":") && is(16, b":")) {
        return None;
    }
    if !(1..=12).contains(&month) || hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    let days = crate::util::days_from_civil(year, month, day);
    if crate::util::civil_from_days(days) != (year, month, day) {
        return None;
    }

    let mut at = 19;
    if is(at, b".") {
        at += 1;
        let fraction = bytes
            .get(at..)?
            .iter()
            .take_while(|byte| byte.is_ascii_digit())
            .count();
        if fraction == 0 {
            return None;
        }
        at += fraction;
    }
    let offset_seconds = if is(at, b"Zz") {
        at += 1;
        0
    } else {
        let sign = if is(at, b"+") {
            1
        } else if is(at, b"-") {
            -1
        } else {
            return None;
        };
        let offset_hour = digits(at + 1, 2)?;
        let offset_minute = digits(at + 4, 2)?;
        if !is(at + 3, b":") || offset_hour > 23 || offset_minute > 59 {
            return None;
        }
        at += 6;
        sign * (offset_hour * 3600 + offset_minute * 60)
    };
    if at != bytes.len() {
        return None;
    }
    let local = days
        .checked_mul(86_400)?
        .checked_add(hour * 3600 + minute * 60 + second)?;
    u64::try_from(local.checked_sub(offset_seconds)?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encoded(report: &UsageReport) -> String {
        report.encode().expect("a report within its bounds encodes")
    }

    fn report() -> UsageReport {
        UsageReport {
            pool: "anthropic-api".to_owned(),
            provider: "anthropic-cost".to_owned(),
            fetched_at: 1_795_435_380,
            windows: vec![UsageWindow {
                label: "month".to_owned(),
                used: 12.5,
                resets_at: Some(1_796_083_200),
            }],
        }
    }

    #[test]
    fn a_report_survives_its_own_encoding() {
        let written = encoded(&report());
        assert!(written.ends_with('\n'), "{written:?}");
        assert_eq!(UsageReport::decode(written.as_bytes()), Ok(report()));

        let mut undated = report();
        undated.windows[0].resets_at = None;
        assert_eq!(
            UsageReport::decode(encoded(&undated).as_bytes()),
            Ok(undated)
        );
    }

    #[test]
    fn the_encoding_is_the_schema_the_design_documents() {
        let value: serde_json::Value =
            serde_json::from_str(&encoded(&report())).expect("the encoding is JSON");
        assert_eq!(
            value,
            serde_json::json!({
                "schema": 1,
                "pool": "anthropic-api",
                "provider": "anthropic-cost",
                "fetched_at": "2026-11-23T12:03:00Z",
                "windows": [
                    {"label": "month", "used_amount": 12.5, "resets_at": "2026-12-01T00:00:00Z"},
                ],
            })
        );
    }

    #[test]
    fn a_report_from_a_newer_schema_is_refused_as_that_schema() {
        let newer = r#"{"schema": 2, "pool": "p", "something_new": true}"#;
        assert_eq!(
            UsageReport::decode(newer.as_bytes()),
            Err(ReportDefect::Schema { found: 2 })
        );
    }

    #[test]
    fn an_unknown_field_a_missing_field_and_a_torn_file_are_malformed() {
        let whole = encoded(&report());
        let with_unknown = whole.replacen("\"schema\"", "\"extra\": 1, \"schema\"", 1);
        let without_pool = whole.replacen("\"pool\": \"anthropic-api\",", "", 1);
        let without_reading = whole.replacen("\"used_amount\": 12.5,", "", 1);
        let torn: String = whole.chars().take(whole.len() / 2).collect();
        for (case, text) in [
            ("unknown field", with_unknown),
            ("missing field", without_pool),
            ("a window with no reading", without_reading),
            ("torn file", torn),
            ("not json", "pool = \"anthropic-api\"".to_owned()),
            ("empty", String::new()),
        ] {
            assert_ne!(text, whole, "{case}: the fixture edit did not apply");
            assert!(
                matches!(
                    UsageReport::decode(text.as_bytes()),
                    Err(ReportDefect::Malformed(_))
                ),
                "{case}: {:?}",
                UsageReport::decode(text.as_bytes())
            );
        }
    }

    #[test]
    fn a_reading_out_of_range_is_refused_on_the_way_in_and_on_the_way_out() {
        for used in [-1.0, f64::NAN, f64::INFINITY] {
            let mut bad = report();
            bad.windows[0].used = used;
            assert!(
                matches!(bad.validate(), Err(ReportDefect::Window { index: 1, .. })),
                "{used:?} validated"
            );
            assert_eq!(
                bad.encode(),
                bad.validate().map(|()| String::new()),
                "{used:?}: what does not validate does not encode"
            );
        }
        let written = encoded(&report()).replacen("12.5", "-12.5", 1);
        assert!(matches!(
            UsageReport::decode(written.as_bytes()),
            Err(ReportDefect::Window { index: 1, .. })
        ));
    }

    #[test]
    fn a_timestamp_that_is_not_one_is_refused_where_it_sits() {
        let whole = encoded(&report());
        let bad_fetch = whole.replacen("2026-11-23T12:03:00Z", "yesterday", 1);
        assert!(matches!(
            UsageReport::decode(bad_fetch.as_bytes()),
            Err(ReportDefect::Field {
                field: "fetched_at",
                ..
            })
        ));
        let bad_reset = whole.replacen("2026-12-01T00:00:00Z", "2026-12-01", 1);
        assert!(matches!(
            UsageReport::decode(bad_reset.as_bytes()),
            Err(ReportDefect::Window { index: 1, .. })
        ));
    }

    #[test]
    fn names_labels_and_window_counts_are_bounded() {
        let mut blank_pool = report();
        blank_pool.pool = "  ".to_owned();
        assert!(matches!(
            blank_pool.validate(),
            Err(ReportDefect::Field { field: "pool", .. })
        ));

        let mut control_in_provider = report();
        control_in_provider.provider = "x\u{1b}[2J".to_owned();
        assert!(matches!(
            control_in_provider.validate(),
            Err(ReportDefect::Field {
                field: "provider",
                ..
            })
        ));

        let mut long_label = report();
        long_label.windows[0].label = "w".repeat(MAX_LABEL_CHARS + 1);
        assert!(matches!(
            long_label.validate(),
            Err(ReportDefect::Window { index: 1, .. })
        ));

        let mut crowded = report();
        crowded.windows = vec![crowded.windows[0].clone(); MAX_WINDOWS + 1];
        assert!(matches!(
            crowded.validate(),
            Err(ReportDefect::Field {
                field: "windows",
                ..
            })
        ));
    }

    #[test]
    fn two_pool_names_one_reduction_spells_alike_get_two_files() {
        let store = Path::new("store");
        let slash = report_path(store, "team/claude");
        let dash = report_path(store, "team-claude");
        assert_ne!(slash, dash);
        for path in [&slash, &dash] {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .expect("a UTF-8 file name");
            assert!(name.starts_with("team-claude-"), "{name}");
            assert!(name.ends_with(".json"), "{name}");
            assert_eq!(path.parent(), Some(store));
        }
        assert_eq!(
            report_path(store, "team/claude"),
            slash,
            "the name is stable"
        );
    }

    #[test]
    fn rfc3339_reads_the_event_logs_form_and_the_providers_form() {
        for (text, unix) in [
            ("1970-01-01T00:00:00Z", 0),
            ("2026-11-23T12:03:00Z", 1_795_435_380),
            ("2026-11-23t12:03:00z", 1_795_435_380),
            ("2026-11-23T12:03:00.943648+00:00", 1_795_435_380),
            ("2026-11-23T14:03:00+02:00", 1_795_435_380),
            ("2026-11-23T06:33:00-05:30", 1_795_435_380),
            ("2024-02-29T00:00:00Z", 1_709_164_800),
        ] {
            assert_eq!(unix_from_rfc3339(text), Some(unix), "{text}");
        }
        assert_eq!(
            crate::util::rfc3339_utc(1_795_435_380),
            "2026-11-23T12:03:00Z",
            "the fixture above is the moment it claims to be"
        );
        assert_eq!(
            crate::util::rfc3339_utc(1_796_083_200),
            "2026-12-01T00:00:00Z"
        );
    }

    #[test]
    fn rfc3339_refuses_what_is_not_a_moment() {
        for text in [
            "",
            "2026-11-23",
            "2026-11-23T12:03:00",
            "2026-11-23 12:03:00Z",
            "2026-13-01T00:00:00Z",
            "2026-02-30T00:00:00Z",
            "2025-02-29T00:00:00Z",
            "2026-11-23T24:00:00Z",
            "2026-11-23T12:60:00Z",
            "2026-11-23T12:03:60Z",
            "2026-11-23T12:03:00.Z",
            "2026-11-23T12:03:00+0200",
            "2026-11-23T12:03:00+24:00",
            "2026-11-23T12:03:00Zjunk",
            "1969-12-31T23:59:59Z",
            "1970-01-01T00:00:00+00:01",
            "٢٠٢٦-11-23T12:03:00Z",
        ] {
            assert_eq!(unix_from_rfc3339(text), None, "{text:?}");
        }
    }
}
