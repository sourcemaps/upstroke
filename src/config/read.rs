//! Extended notes: `docs/internals/config/read.md`

#![forbid(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

use super::{
    Allowance, FileSnapshot, Path, PathBuf, Pool, PoolKind, RawPool, RawPools, RawRepoConfig,
    Source, UpstrokeError, capacity,
};
use crate::capacity::UsageSource;

pub(super) fn read_repo_config(
    snapshot: &FileSnapshot,
) -> Result<(RawRepoConfig, PathBuf), UpstrokeError> {
    let path = snapshot.path().to_path_buf();
    let Some(text) = snapshot.text()? else {
        if snapshot.required {
            return Err(UpstrokeError::Config {
                path,
                message: "file not found".to_owned(),
            });
        }
        return Ok((RawRepoConfig::default(), path));
    };
    match toml::from_str(&text) {
        Ok(raw) => Ok((raw, path)),
        Err(e) => Err(UpstrokeError::Config {
            path,
            message: toml_error_message(&e, Some(&text)),
        }),
    }
}

pub(super) struct MachineSettings {
    pub(super) raw: RawRepoConfig,
    pub(super) table: toml::Table,
    pub(super) path: PathBuf,
}

pub(super) fn read_machine_settings(
    snapshot: &FileSnapshot,
) -> Result<Option<MachineSettings>, UpstrokeError> {
    let path = snapshot.path().to_path_buf();
    let Some(text) = snapshot.text()? else {
        return Ok(None);
    };
    let refused = |error: toml::de::Error| {
        let error = toml_error_message(&error, Some(&text));
        UpstrokeError::Config {
            path: path.clone(),
            message: format!(
                "{error} (this is the machine-level settings file, read for every repository on \
                 this machine)"
            ),
        }
    };
    let raw: RawRepoConfig = toml::from_str(&text).map_err(refused)?;
    let table: toml::Table = toml::from_str(&text).map_err(refused)?;
    Ok(Some(MachineSettings { raw, table, path }))
}

pub(super) fn settings_table(snapshot: &FileSnapshot) -> Result<toml::Table, UpstrokeError> {
    let Some(text) = snapshot.text()? else {
        return Ok(toml::Table::new());
    };
    toml::from_str(&text).map_err(|error: toml::de::Error| UpstrokeError::Config {
        path: snapshot.path().to_path_buf(),
        message: toml_error_message(&error, Some(&text)),
    })
}

pub(super) fn read_pools(
    pools: Option<&FileSnapshot>,
    has_adapter: &dyn Fn(&str) -> bool,
    warnings: &mut Vec<String>,
) -> Result<Vec<Pool>, UpstrokeError> {
    let Some(snapshot) = pools else {
        return Ok(Vec::new());
    };
    let path = snapshot.path().to_path_buf();
    let Some(text) = snapshot.text()? else {
        if snapshot.required {
            return Err(UpstrokeError::Config {
                path,
                message: "pools file not found".to_owned(),
            });
        }
        return Ok(Vec::new());
    };
    let raw: RawPools = match toml::from_str(&text) {
        Ok(raw) => raw,
        Err(e) => {
            return Err(UpstrokeError::Config {
                path,
                message: toml_error_message(&e, Some(&text)),
            });
        }
    };
    let mut entries: Vec<(String, toml::Spanned<toml::Value>)> =
        raw.pools.unwrap_or_default().into_iter().collect();
    entries.sort_by_key(|(_, spanned)| spanned.span().start);
    let mut pools = Vec::new();
    for (name, spanned) in entries {
        pools.push(parse_pool(
            &name,
            spanned.into_inner(),
            &path,
            has_adapter,
            warnings,
        )?);
    }
    Ok(pools)
}

fn parse_pool(
    name: &str,
    value: toml::Value,
    path: &Path,
    has_adapter: &dyn Fn(&str) -> bool,
    warnings: &mut Vec<String>,
) -> Result<Pool, UpstrokeError> {
    let config_error = |message: String| UpstrokeError::Config {
        path: path.to_path_buf(),
        message,
    };
    if name.trim().is_empty() {
        return Err(config_error(
            "a pool needs a non-empty name — `[pools.<name>]` is what attempts are attributed to"
                .to_owned(),
        ));
    }
    let raw: RawPool = value.try_into().map_err(|e| {
        let e = toml_error_message(&e, None);
        config_error(format!(
            "[pools.{name}]: {e} (expected `kind` and `agent` strings, with optional `window`, \
             `weekly`, `sources`, `safety_margin`, `reserve`, `monthly_allowance`, `endpoint`, \
             `profile`, `usage_provider`, `usage_token_env`, and `usage_account`)"
        ))
    })?;

    let kind_text = raw.kind.ok_or_else(|| {
        config_error(format!(
            "[pools.{name}] has no `kind` — one of: {}",
            PoolKind::ACCEPTED.join(", ")
        ))
    })?;
    let kind = PoolKind::parse(&kind_text).ok_or_else(|| {
        config_error(format!(
            "[pools.{name}] `kind = \"{kind_text}\"` is not recognized (accepted: {})",
            PoolKind::ACCEPTED.join(", ")
        ))
    })?;
    let agent = raw
        .agent
        .ok_or_else(|| config_error(format!("[pools.{name}] has no `agent`")))?;

    let window = match raw.window {
        None => None,
        Some(text) => Some(capacity::parse_duration(&text).ok_or_else(|| {
            config_error(format!(
                "[pools.{name}] `window = \"{text}\"` is not a duration — write a number and one \
                 of s, m, h, d (for example \"5h\")"
            ))
        })?),
    };

    let mut sources = Vec::new();
    for entry in raw.sources.unwrap_or_default() {
        let source = Source::parse(&entry).ok_or_else(|| {
            config_error(format!(
                "[pools.{name}] `sources` entry `{entry}` is not recognized (accepted: {})",
                Source::ACCEPTED.join(", ")
            ))
        })?;
        if !sources.contains(&source) {
            sources.push(source);
        }
    }

    let fraction = |field: &str, value: Option<f64>, default: f64| -> Result<f64, UpstrokeError> {
        let Some(value) = value else {
            return Ok(default);
        };
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(config_error(format!(
                "[pools.{name}] `{field} = {value}` is out of range — it is a fraction of the \
                 pool between 0.0 and 1.0"
            )));
        }
        Ok(value)
    };
    let safety_margin = fraction(
        "safety_margin",
        raw.safety_margin,
        capacity::DEFAULT_SAFETY_MARGIN,
    )?;
    let reserve = fraction("reserve", raw.reserve, capacity::DEFAULT_RESERVE)?;

    let monthly_allowance = match raw.monthly_allowance {
        None => Allowance::Auto,
        Some(toml::Value::String(text)) if text.trim().eq_ignore_ascii_case("auto") => {
            Allowance::Auto
        }
        Some(toml::Value::Integer(units)) => {
            if !converts_exactly(units) {
                return Err(config_error(format!(
                    "[pools.{name}] `monthly_allowance = {units}` cannot be held as written — an \
                     allowance is a 64-bit float, which carries {} significant bits, and this \
                     integer needs more; write a value that converts without change",
                    f64::MANTISSA_DIGITS
                )));
            }
            Allowance::Units(units as f64)
        }
        Some(toml::Value::Float(units)) => Allowance::Units(units),
        Some(other) => {
            return Err(config_error(format!(
                "[pools.{name}] `monthly_allowance` must be a number of units or the string \
                 \"auto\", found a {}",
                other.type_str()
            )));
        }
    };
    if let Allowance::Units(units) = monthly_allowance {
        if !units.is_finite() || units <= 0.0 {
            return Err(config_error(format!(
                "[pools.{name}] `monthly_allowance = {units}` is not an allowance — write \"auto\" if \
                 you do not know its size"
            )));
        }
    }

    for key in raw.unknown.keys() {
        warnings.push(format!(
            "unknown key `{key}` in [pools.{name}] in {} (ignored)",
            path.display()
        ));
    }

    let usage = UsageSource::from_keys(
        raw.usage_provider.as_deref(),
        raw.usage_token_env.as_deref(),
        raw.usage_account.as_deref(),
    )
    .map_err(|defect| config_error(format!("[pools.{name}] {defect}")))?;
    if let Some(usage) = &usage {
        for gap in usage.gaps() {
            warnings.push(format!("[pools.{name}] {gap}"));
        }
        if !sources.contains(&Source::ProviderEndpoint) {
            sources.push(Source::ProviderEndpoint);
        }
    }

    let usable = has_adapter(&agent);
    if !usable {
        warnings.push(format!(
            "[pools.{name}] names agent `{agent}`, which has no adapter in this build — the pool \
             is listed but this engine can never draw from it"
        ));
    }

    Ok(Pool {
        name: name.to_owned(),
        kind,
        agent,
        window,
        weekly: raw.weekly.unwrap_or(false),
        sources,
        safety_margin,
        reserve,
        monthly_allowance,
        endpoint: raw.endpoint,
        profile: raw.profile,
        usage,
        usable,
    })
}

fn converts_exactly(units: i64) -> bool {
    let magnitude = units.unsigned_abs();
    magnitude == 0
        || u64::BITS - magnitude.leading_zeros() - magnitude.trailing_zeros()
            <= f64::MANTISSA_DIGITS
}

pub(super) fn toml_error_message(error: &toml::de::Error, text: Option<&str>) -> String {
    let message = redact_quoted(error.message());
    match (error.span(), text) {
        (Some(span), Some(text)) => {
            let (line, column) = position(text, span.start);
            format!("line {line}, column {column}: {message}")
        }
        (Some(_), None) | (None, _) => message,
    }
}

fn position(text: &str, offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut column = 1;
    for (at, character) in text.char_indices() {
        if at >= offset {
            break;
        }
        if character == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}

const LONGEST_QUOTED_VALUE: usize = 16;

const LONGEST_QUOTED_NAME: usize = 64;

const NAME_FOLLOWS: [&str; 5] = [
    "unknown field ",
    "missing field ",
    "duplicate field ",
    "duplicate key ",
    "dotted key ",
];

const LIST_FOLLOWS: [&str; 2] = ["unknown field ", "unknown variant "];

#[derive(Clone, Copy)]
enum Previous {
    Other,
    Subject,
    Entry,
}

fn redact_quoted(message: &str) -> String {
    let mut out = String::with_capacity(message.len());
    let mut plain = String::new();
    let mut previous = Previous::Other;
    let mut characters = message.chars();
    while let Some(character) = characters.next() {
        if !matches!(character, '"' | '\'' | '`') {
            plain.push(character);
            continue;
        }
        let quote = character;
        let mut quoted = String::new();
        let mut closed = false;
        while let Some(inner) = characters.next() {
            if inner == quote {
                closed = true;
                break;
            }
            quoted.push(inner);
            if quote == '"' && inner == '\\' {
                if let Some(escaped) = characters.next() {
                    quoted.push(escaped);
                }
            }
        }
        let (longest, next) = slot(quote, &plain, previous);
        previous = next;
        out.push_str(&plain);
        plain.clear();
        out.push(quote);
        let count = quoted.chars().count();
        if count > longest {
            out.push_str(&format!("<redacted: {count} characters>"));
        } else {
            out.push_str(&quoted);
        }
        if closed {
            out.push(quote);
        }
    }
    out.push_str(&plain);
    out
}

fn slot(quote: char, plain: &str, previous: Previous) -> (usize, Previous) {
    if quote != '`' {
        return (LONGEST_QUOTED_VALUE, Previous::Other);
    }
    let continues_list = match previous {
        Previous::Subject => plain == ", expected " || plain == ", expected one of ",
        Previous::Entry => plain == ", " || plain == " or ",
        Previous::Other => false,
    };
    if continues_list {
        return (LONGEST_QUOTED_NAME, Previous::Entry);
    }
    let longest = if NAME_FOLLOWS.iter().any(|lead| plain.ends_with(lead)) {
        LONGEST_QUOTED_NAME
    } else {
        LONGEST_QUOTED_VALUE
    };
    let next = if LIST_FOLLOWS.iter().any(|lead| plain.ends_with(lead)) {
        Previous::Subject
    } else {
        Previous::Other
    };
    (longest, next)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pool_value(body: &str) -> toml::Value {
        toml::from_str(body).expect("valid pool body")
    }

    fn any_adapter(_: &str) -> bool {
        true
    }

    fn absent(tag: &str, required: bool) -> FileSnapshot {
        FileSnapshot {
            path: PathBuf::from(format!("absent-{tag}.toml")),
            required,
            content: Ok(None),
        }
    }

    fn holding(name: &str, text: &str) -> FileSnapshot {
        FileSnapshot {
            path: PathBuf::from(name),
            required: true,
            content: Ok(Some(text.as_bytes().to_vec())),
        }
    }

    fn refusal_of_repo_file(text: &str) -> String {
        match read_repo_config(&holding("upstroke.toml", text)) {
            Err(UpstrokeError::Config { message, .. }) => message,
            Err(_) => panic!("the refusal is a configuration error"),
            Ok(_) => panic!("the file is refused"),
        }
    }

    #[test]
    fn read_repo_config_of_a_required_absent_file_is_an_error() {
        let snapshot = absent("required", true);
        let err = read_repo_config(&snapshot).expect_err("a required file that is absent errors");
        assert!(matches!(err, UpstrokeError::Config { .. }));
        assert!(err.to_string().contains("file not found"));
    }

    #[test]
    fn read_repo_config_of_an_optional_absent_file_is_the_default() {
        let snapshot = absent("optional", false);
        let (raw, returned_path) = read_repo_config(&snapshot).expect("absent optional defaults");
        assert_eq!(returned_path.as_path(), snapshot.path());
        assert!(raw.routing.is_none());
        assert!(raw.pins.is_none());
    }

    #[test]
    fn a_blank_pool_name_is_refused() {
        let path = Path::new("pools.toml");
        let mut warnings = Vec::new();
        let err = parse_pool(
            "   ",
            pool_value("kind = \"credits\"\nagent = \"claude-code\"\n"),
            path,
            &any_adapter,
            &mut warnings,
        )
        .expect_err("a blank name is refused");
        assert!(err.to_string().contains("non-empty name"));
    }

    #[test]
    fn an_unrecognized_kind_is_refused_naming_the_accepted_set() {
        let path = Path::new("pools.toml");
        let mut warnings = Vec::new();
        let err = parse_pool(
            "p",
            pool_value("kind = \"subscription\"\nagent = \"claude-code\"\n"),
            path,
            &any_adapter,
            &mut warnings,
        )
        .expect_err("an unrecognized kind is refused");
        assert!(err.to_string().contains("subscription"));
    }

    #[test]
    fn a_missing_agent_is_refused() {
        let path = Path::new("pools.toml");
        let mut warnings = Vec::new();
        let err = parse_pool(
            "p",
            pool_value("kind = \"credits\"\n"),
            path,
            &any_adapter,
            &mut warnings,
        )
        .expect_err("a pool with no agent is refused");
        assert!(err.to_string().contains("no `agent`"));
    }

    #[test]
    fn an_unparseable_window_is_refused() {
        let path = Path::new("pools.toml");
        let mut warnings = Vec::new();
        let err = parse_pool(
            "p",
            pool_value(
                "kind = \"subscription-window\"\nagent = \"claude-code\"\nwindow = \"soon\"\n",
            ),
            path,
            &any_adapter,
            &mut warnings,
        )
        .expect_err("an unparseable window is refused");
        assert!(err.to_string().contains("soon"));
    }

    #[test]
    fn safety_margin_outside_zero_to_one_is_refused() {
        let path = Path::new("pools.toml");
        let mut warnings = Vec::new();
        let err = parse_pool(
            "p",
            pool_value("kind = \"credits\"\nagent = \"claude-code\"\nsafety_margin = 1.5\n"),
            path,
            &any_adapter,
            &mut warnings,
        )
        .expect_err("an out-of-range safety_margin is refused");
        assert!(err.to_string().contains("safety_margin"));
    }

    #[test]
    fn reserve_outside_zero_to_one_is_refused() {
        let path = Path::new("pools.toml");
        let mut warnings = Vec::new();
        let err = parse_pool(
            "p",
            pool_value("kind = \"credits\"\nagent = \"claude-code\"\nreserve = -0.1\n"),
            path,
            &any_adapter,
            &mut warnings,
        )
        .expect_err("a negative reserve is refused");
        assert!(err.to_string().contains("reserve"));
    }

    #[test]
    fn monthly_allowance_accepts_auto_case_and_whitespace_insensitively() {
        let path = Path::new("pools.toml");
        let mut warnings = Vec::new();
        let pool = parse_pool(
            "p",
            pool_value(
                "kind = \"credits\"\nagent = \"claude-code\"\nmonthly_allowance = \" AUTO \"\n",
            ),
            path,
            &any_adapter,
            &mut warnings,
        )
        .expect("auto, any case and padding, is accepted");
        assert_eq!(pool.monthly_allowance, Allowance::Auto);
    }

    #[test]
    fn monthly_allowance_accepts_an_integer_as_units() {
        let path = Path::new("pools.toml");
        let mut warnings = Vec::new();
        let pool = parse_pool(
            "p",
            pool_value("kind = \"credits\"\nagent = \"claude-code\"\nmonthly_allowance = 300\n"),
            path,
            &any_adapter,
            &mut warnings,
        )
        .expect("an integer allowance is accepted");
        assert_eq!(pool.monthly_allowance, Allowance::Units(300.0));
    }

    #[test]
    fn monthly_allowance_accepts_a_float_as_units() {
        let path = Path::new("pools.toml");
        let mut warnings = Vec::new();
        let pool = parse_pool(
            "p",
            pool_value("kind = \"credits\"\nagent = \"claude-code\"\nmonthly_allowance = 12.5\n"),
            path,
            &any_adapter,
            &mut warnings,
        )
        .expect("a float allowance is accepted");
        assert_eq!(pool.monthly_allowance, Allowance::Units(12.5));
    }

    #[test]
    fn an_integer_allowance_that_converts_exactly_is_accepted_unchanged() {
        let path = Path::new("pools.toml");
        for (written, expected) in [
            ("9007199254740992", 9_007_199_254_740_992.0),
            ("9007199254740994", 9_007_199_254_740_994.0),
            ("10000000000000000", 10_000_000_000_000_000.0),
        ] {
            let mut warnings = Vec::new();
            let pool = parse_pool(
                "p",
                pool_value(&format!(
                    "kind = \"credits\"\nagent = \"claude-code\"\nmonthly_allowance = {written}\n"
                )),
                path,
                &any_adapter,
                &mut warnings,
            )
            .expect("an integer that converts exactly is an allowance");
            assert_eq!(
                pool.monthly_allowance,
                Allowance::Units(expected),
                "{written} survives the cast as the number that was written"
            );
        }
    }

    #[test]
    fn an_integer_allowance_the_cast_would_change_is_refused() {
        let path = Path::new("pools.toml");
        for units in [9_007_199_254_740_993_i64, i64::MAX] {
            let mut warnings = Vec::new();
            let err = parse_pool(
                "p",
                pool_value(&format!(
                    "kind = \"credits\"\nagent = \"claude-code\"\nmonthly_allowance = {units}\n"
                )),
                path,
                &any_adapter,
                &mut warnings,
            )
            .expect_err("an allowance past the exactly-representable range is refused");
            assert!(err.to_string().contains("monthly_allowance"), "got: {err}");
        }
    }

    #[test]
    fn monthly_allowance_rejects_zero_and_negative_and_non_finite() {
        let path = Path::new("pools.toml");
        for body in [
            "kind = \"credits\"\nagent = \"claude-code\"\nmonthly_allowance = 0\n",
            "kind = \"credits\"\nagent = \"claude-code\"\nmonthly_allowance = -5\n",
            "kind = \"credits\"\nagent = \"claude-code\"\nmonthly_allowance = nan\n",
            "kind = \"credits\"\nagent = \"claude-code\"\nmonthly_allowance = inf\n",
        ] {
            let mut warnings = Vec::new();
            let err = parse_pool("p", pool_value(body), path, &any_adapter, &mut warnings)
                .expect_err(&format!("{body} is not a usable allowance"));
            assert!(err.to_string().contains("monthly_allowance"), "got: {err}");
        }
    }

    #[test]
    fn monthly_allowance_of_the_wrong_shape_names_the_type_it_saw() {
        let path = Path::new("pools.toml");
        let mut warnings = Vec::new();
        let err = parse_pool(
            "p",
            pool_value("kind = \"credits\"\nagent = \"claude-code\"\nmonthly_allowance = true\n"),
            path,
            &any_adapter,
            &mut warnings,
        )
        .expect_err("a boolean allowance is refused");
        assert!(err.to_string().contains("monthly_allowance"));
    }

    #[test]
    fn duplicate_sources_entries_are_deduplicated_in_file_order() {
        let path = Path::new("pools.toml");
        let mut warnings = Vec::new();
        let pool = parse_pool(
            "p",
            pool_value(
                "kind = \"credits\"\nagent = \"claude-code\"\nsources = [\"signals\", \"signals\"]\n",
            ),
            path,
            &any_adapter,
            &mut warnings,
        )
        .expect("duplicate sources parse");
        assert_eq!(pool.sources.len(), 1);
    }

    #[test]
    fn an_agent_with_no_adapter_warns_and_the_pool_stays_but_unusable() {
        let path = Path::new("pools.toml");
        let mut warnings = Vec::new();
        let pool = parse_pool(
            "p",
            pool_value("kind = \"credits\"\nagent = \"nonexistent\"\n"),
            path,
            &|_: &str| false,
            &mut warnings,
        )
        .expect("a pool for an unknown adapter is still a pool");
        assert!(!pool.usable);
        assert!(warnings.iter().any(|w| w.contains("nonexistent")));
    }

    #[test]
    fn an_unknown_key_warns_by_name_and_does_not_fail_the_pool() {
        let path = Path::new("pools.toml");
        let mut warnings = Vec::new();
        let pool = parse_pool(
            "p",
            pool_value("kind = \"credits\"\nagent = \"claude-code\"\nfrobnicate = true\n"),
            path,
            &any_adapter,
            &mut warnings,
        )
        .expect("an unknown key degrades to a warning, not a refusal");
        assert!(pool.usable);
        assert!(warnings.iter().any(|w| w.contains("frobnicate")));
    }

    #[test]
    fn unquoted_pasted_token_is_not_echoed_by_configuration_error() {
        use crate::config::{CapturedConfig, EngineLimits, load_captured};

        let token = format!("github_pat_{}", "Z".repeat(82));
        let pasted = format!("usage_token_env = {token}\n");
        for (file, captured, at) in [
            (
                "pools.toml",
                CapturedConfig {
                    repo: absent("witness", false),
                    machine: None,
                    pools: Some(holding(
                        "pools.toml",
                        &format!("[pools.p]\nkind = \"credits\"\n{pasted}"),
                    )),
                },
                "line 3, column 19: ",
            ),
            (
                "upstroke.toml",
                CapturedConfig {
                    repo: holding("upstroke.toml", &format!("[engine]\n{pasted}")),
                    machine: None,
                    pools: None,
                },
                "line 2, column 19: ",
            ),
        ] {
            let mut warnings = Vec::new();
            let loaded = load_captured(&captured, EngineLimits::Topology, &mut warnings);
            let Err(refused) = loaded else {
                panic!("{file}: a value that is not TOML is refused");
            };
            assert!(
                !refused.to_string().contains(&token),
                "{file}: the refusal must not repeat the pasted token"
            );
            let UpstrokeError::Config { path, message } = &refused else {
                panic!("{file}: the refusal is a configuration error");
            };
            assert_eq!(path, Path::new(file), "the refusal names the file");
            assert!(
                message.starts_with(at),
                "{file}: the refusal names the line and the column, {at:?}"
            );
            assert!(
                message.contains("invalid string"),
                "{file}: the refusal still says what is wrong"
            );
        }
    }

    #[test]
    fn a_long_quoted_value_in_a_type_error_is_redacted_and_a_short_one_is_kept() {
        for (written, said) in [
            ("abcdefghijklmnop", "\"abcdefghijklmnop\""),
            ("abcdefghijklmnopq", "\"<redacted: 17 characters>\""),
        ] {
            assert_eq!(
                refusal_of_repo_file(&format!("pins = \"{written}\"\n")),
                format!("line 1, column 8: invalid type: string {said}, expected a sequence")
            );
            let mut warnings = Vec::new();
            let refused = parse_pool(
                "p",
                pool_value(&format!(
                    "kind = \"credits\"\nagent = \"claude-code\"\nreserve = \"{written}\"\n"
                )),
                Path::new("pools.toml"),
                &any_adapter,
                &mut warnings,
            )
            .expect_err("a string where a fraction belongs is refused");
            let expected = format!("[pools.p]: invalid type: string {said}, expected f64 (");
            assert!(
                refused.to_string().contains(&expected),
                "{expected:?}: {refused}"
            );
        }

        let token = format!("ghp_{}", "Z".repeat(36));
        let mut warnings = Vec::new();
        let Err(refused) = parse_pool(
            "p",
            pool_value(&format!(
                "kind = \"credits\"\nagent = \"claude-code\"\nreserve = \"{token}\"\n"
            )),
            Path::new("pools.toml"),
            &any_adapter,
            &mut warnings,
        ) else {
            panic!("a string where a fraction belongs is refused");
        };
        let refused = refused.to_string();
        assert!(
            !refused.contains(&token),
            "the refusal must not repeat a pasted token"
        );
        assert!(
            refused.contains("invalid type: string \"<redacted: 40 characters>\", expected f64"),
            "the refusal still says what is wrong"
        );
    }

    #[test]
    fn a_seventeen_character_unknown_key_is_named() {
        for key in ["interaction_modes".to_owned(), "k".repeat(64)] {
            let message = refusal_of_repo_file(&format!("{key} = 1\n"));
            assert!(
                message.starts_with(&format!("line 1, column 1: unknown field `{key}`, ")),
                "{message}"
            );
            assert!(
                message.contains("`interaction`") && message.contains("`capacity`"),
                "the refusal lists the accepted sections: {message}"
            );
        }
    }

    #[test]
    fn a_sixty_five_character_unknown_key_is_redacted() {
        let key = format!("github_pat_{}", "Z".repeat(54));
        let message = refusal_of_repo_file(&format!("{key} = 1\n"));
        assert!(
            !message.contains(&key),
            "a pasted credential used as a key must not be repeated"
        );
        assert!(
            message.starts_with(
                "line 1, column 1: unknown field `<redacted: 65 characters>`, expected one of "
            ),
            "the refusal still says what is wrong and where"
        );
        assert!(
            message.contains("`interaction`") && message.contains("`capacity`"),
            "the refusal still lists the accepted sections"
        );
    }

    #[test]
    fn an_ordinary_syntax_error_says_what_is_wrong_and_where() {
        let message = refusal_of_repo_file("[engine]\nmax_parallel = 2\n[gates\n");
        assert!(message.starts_with("line 3, column 7: "), "{message}");
        assert!(message.contains("table header"), "{message}");
        assert!(
            !message.contains("[gates"),
            "the refusal never repeats the file: {message}"
        );
    }

    #[test]
    fn a_machine_level_file_error_keeps_its_suffix() {
        let snapshot = holding(
            "machine/upstroke.toml",
            "[engine]\nmax_parallel = 2\n[gates\n",
        );
        let Err(refused) = read_machine_settings(&snapshot) else {
            panic!("a machine-level file that is not TOML is refused");
        };
        let UpstrokeError::Config { path, message } = &refused else {
            panic!("a configuration error: {refused}");
        };
        assert_eq!(path, snapshot.path());
        assert!(message.starts_with("line 3, column 7: "), "{message}");
        assert!(message.contains("table header"), "{message}");
        assert!(
            message.ends_with(
                " (this is the machine-level settings file, read for every repository on this \
                 machine)"
            ),
            "{message}"
        );
    }

    #[test]
    fn a_quoted_run_is_measured_by_where_it_stands() {
        for (message, expected) in [
            (
                "invalid type: string \"ab\\\"cdefghijklmnopqr\", expected u64",
                "invalid type: string \"<redacted: 20 characters>\", expected u64",
            ),
            (
                "invalid value: integer `-1`, expected u64",
                "invalid value: integer `-1`, expected u64",
            ),
            (
                "unknown variant `abcdefghijklmnopq`, expected one of `small`, `mid`, `frontier`",
                "unknown variant `<redacted: 17 characters>`, expected one of `small`, `mid`, \
                 `frontier`",
            ),
            (
                "unknown field `frontier_escalation`, expected `frontier_escalation_over_usd`",
                "unknown field `frontier_escalation`, expected `frontier_escalation_over_usd`",
            ),
            (
                "unknown field `notify_channels`, expected `notify` or `wait_on_block_secs`",
                "unknown field `notify_channels`, expected `notify` or `wait_on_block_secs`",
            ),
            (
                "missing field `frontier_escalation_over_usd`",
                "missing field `frontier_escalation_over_usd`",
            ),
            (
                "duplicate key `frontier_escalation` in table `interaction.ask_before`",
                "duplicate key `frontier_escalation` in table `<redacted: 22 characters>`",
            ),
            (
                "dotted key `routing.strategy.mode` attempted to extend non-table type (string)",
                "dotted key `routing.strategy.mode` attempted to extend non-table type (string)",
            ),
            (
                "invalid type: string \"wait_on_block_secs\", expected `wait_on_block_secs`",
                "invalid type: string \"<redacted: 18 characters>\", expected \
                 `<redacted: 18 characters>`",
            ),
            (
                "invalid string\nexpected `\"`, `'`",
                "invalid string\nexpected `\"`, `'`",
            ),
            ("expected '`'", "expected '`'"),
            (
                "unexpected `abcdefghijklmnopqrst",
                "unexpected `<redacted: 20 characters>",
            ),
        ] {
            assert_eq!(redact_quoted(message), expected, "{message:?}");
        }
    }
}
