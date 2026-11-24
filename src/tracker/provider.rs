//! The provider usage endpoints the tracker knows: what to ask each one, and
//! how to read what it answers (DESIGN.md §13).
//!
//! Every endpoint here is one its provider **documents** and authenticates
//! with a credential issued for the purpose (an organization admin key, a
//! fine-grained token with a billing permission). That is the admission rule,
//! and it is invariant 5 applied to HTTP: an undocumented endpoint, or one
//! that takes a credential issued to a vendor's own application, is not added
//! here however widely it is used.
//!
//! All four report **spend for the current calendar month**, which is what a
//! billing endpoint knows. None reports what is left; the pool's
//! `monthly_allowance` supplies the other half, on the reading side.
//!
//! Everything in this module is a pure function. The request is a value, the
//! response is a value, and nothing here starts a process: that is
//! `transport`'s, so that every rule below is exercised with no network.
//!
//! **A response is untrusted input** (CODING_STANDARDS.md §9, §14). Each
//! parser reads the fields it needs and nothing else, and refuses rather than
//! guesses: a currency it does not expect, a unit it does not expect, a
//! negative or non-finite number, a page that says more pages follow. Every
//! one of those would otherwise become a spend figure that is too low, and a
//! spend figure that is too low is a ceiling that is too high.

#![forbid(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

use std::fmt;

use serde::Deserialize;

use crate::capacity::UsageProvider;
use crate::capacity::usage::UsageWindow;

/// A credential. Its `Debug` and `Display` never print it, so no error,
/// event or diagnostic that formats a request can leak it (§14).
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    #[must_use]
    pub fn new(value: String) -> Self {
        Self(value)
    }

    /// The credential itself, for the one caller that has to send it.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

/// How a provider takes its credential: the header, and what precedes the
/// secret in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credential {
    pub header: &'static str,
    pub scheme: &'static str,
    pub secret: Secret,
}

/// One HTTPS `GET`. The credential is kept apart from the other headers so
/// that it is the transport, and nothing before it, that puts the secret into
/// a header line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageRequest {
    pub url: String,
    pub headers: Vec<(&'static str, String)>,
    pub credential: Credential,
}

/// What came back: the status, and the body as text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageResponse {
    pub status: u16,
    pub body: String,
}

/// Why a provider gave no reading.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProviderError {
    #[error("`{provider}` reports per account and the pool names no usable `usage_account`")]
    NoAccount { provider: UsageProvider },
    #[error(
        "the provider refused the credential (HTTP {status}); check that the variable named by \
         `usage_token_env` holds a key the endpoint accepts"
    )]
    CredentialRefused { status: u16 },
    #[error(
        "the provider found nothing at that address (HTTP 404); for a per-account endpoint that \
         is a wrong `usage_account`, or a token without the permission to read it"
    )]
    NotFound,
    #[error("the provider is rate limiting the tracker (HTTP 429); ask again later")]
    RateLimited,
    #[error("the provider answered HTTP {status}")]
    Status { status: u16 },
    #[error("the provider's answer could not be read as the documented response: {detail}")]
    Unreadable { detail: String },
    #[error(
        "the provider says more pages follow, so the figure on this one is not the month's \
         total"
    )]
    Partial,
    #[error("the provider reported {found}, where {expected} was expected")]
    Unexpected { expected: String, found: String },
}

const ANTHROPIC_COST: &str = "https://api.anthropic.com/v1/organizations/cost_report";
const ANTHROPIC_VERSION: &str = "2026-10-21";
const OPENAI_COSTS: &str = "https://api.openai.com/v1/organization/costs";
const GITHUB_API: &str = "https://api.github.com";
const GITHUB_API_VERSION: &str = "2026-03-10";

/// A calendar month holds at most 31 daily buckets, so one page of 31 is the
/// whole month and a response claiming more pages is refused.
const DAILY_BUCKETS: u32 = 31;

/// The request that asks `provider` for the current month's usage.
///
/// # Errors
///
/// [`ProviderError::NoAccount`] where the provider reports per account and
/// `account` is absent or is not a plain account name. The configuration
/// reader already refuses such a name; it is checked again here because here
/// is where it becomes part of an address.
pub fn request(
    provider: UsageProvider,
    account: Option<&str>,
    secret: Secret,
    now_unix: u64,
) -> Result<UsageRequest, ProviderError> {
    let month = crate::capacity::machine::month_start(now_unix);
    let bearer = |secret| Credential {
        header: "Authorization",
        scheme: "Bearer ",
        secret,
    };
    match provider {
        UsageProvider::AnthropicCost => Ok(UsageRequest {
            url: format!(
                "{ANTHROPIC_COST}?starting_at={}&bucket_width=1d&limit={DAILY_BUCKETS}",
                crate::util::rfc3339_utc(month)
            ),
            headers: vec![("anthropic-version", ANTHROPIC_VERSION.to_owned())],
            credential: Credential {
                header: "x-api-key",
                scheme: "",
                secret,
            },
        }),
        UsageProvider::OpenaiCost => Ok(UsageRequest {
            url: format!("{OPENAI_COSTS}?start_time={month}&bucket_width=1d&limit={DAILY_BUCKETS}"),
            headers: Vec::new(),
            credential: bearer(secret),
        }),
        UsageProvider::GithubAiCredits | UsageProvider::GithubPremiumRequests => {
            let account = account
                .filter(|account| crate::capacity::names_an_account(account))
                .ok_or(ProviderError::NoAccount { provider })?;
            let product = match provider {
                UsageProvider::GithubAiCredits => "ai_credit",
                _ => "premium_request",
            };
            let (year, month_of_year) = year_and_month(now_unix);
            Ok(UsageRequest {
                url: format!(
                    "{GITHUB_API}/users/{account}/settings/billing/{product}/usage\
                     ?year={year}&month={month_of_year}"
                ),
                headers: vec![
                    ("Accept", "application/vnd.github+json".to_owned()),
                    ("X-GitHub-Api-Version", GITHUB_API_VERSION.to_owned()),
                ],
                credential: bearer(secret),
            })
        }
    }
}

fn year_and_month(now_unix: u64) -> (i64, i64) {
    let days = i64::try_from(now_unix / 86_400).unwrap_or(0);
    let (year, month, _) = crate::util::civil_from_days(days);
    (year, month)
}

/// Unix seconds of 00:00:00 UTC on the first day of the month after `now`'s:
/// when a calendar-month allowance starts over.
#[must_use]
pub fn next_month_start(now_unix: u64) -> u64 {
    let (year, month) = year_and_month(now_unix);
    let (year, month) = if month >= 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    u64::try_from(crate::util::days_from_civil(year, month, 1))
        .map_or(0, |days| days.saturating_mul(86_400))
}

/// The windows `provider`'s answer establishes.
///
/// Each of today's providers yields one: `month`, the amount drawn since the
/// first of the month, resetting at the next first.
///
/// # Errors
///
/// [`ProviderError`] for any status but 200 and for any 200 that is not the
/// documented response in full.
pub fn parse(
    provider: UsageProvider,
    response: &UsageResponse,
    now_unix: u64,
) -> Result<Vec<UsageWindow>, ProviderError> {
    match response.status {
        200 => {}
        401 | 403 => {
            return Err(ProviderError::CredentialRefused {
                status: response.status,
            });
        }
        404 => return Err(ProviderError::NotFound),
        429 => return Err(ProviderError::RateLimited),
        status => return Err(ProviderError::Status { status }),
    }
    let used = match provider {
        UsageProvider::AnthropicCost => anthropic_dollars(&response.body)?,
        UsageProvider::OpenaiCost => openai_dollars(&response.body)?,
        UsageProvider::GithubAiCredits => github_quantity(&response.body, "ai-credits")?,
        UsageProvider::GithubPremiumRequests => github_quantity(&response.body, "requests")?,
    };
    Ok(vec![UsageWindow {
        label: "month".to_owned(),
        used,
        resets_at: Some(next_month_start(now_unix)),
    }])
}

fn read<'a, T: Deserialize<'a>>(body: &'a str) -> Result<T, ProviderError> {
    serde_json::from_str(body).map_err(|error| ProviderError::Unreadable {
        detail: shown(&error.to_string(), 200),
    })
}

/// Text that came from a provider, made fit to print: at most `max`
/// characters, and nothing but printable ASCII. A parser's error can quote
/// the value it choked on, and that value is whatever the far end sent.
fn shown(text: &str, max: usize) -> String {
    let mut out: String = text
        .chars()
        .take(max)
        .map(|c| {
            if c == ' ' || c.is_ascii_graphic() {
                c
            } else {
                '?'
            }
        })
        .collect();
    if text.chars().nth(max).is_some() {
        out.push('…');
    }
    out
}

/// A reported amount, or a refusal: an amount is finite and not negative.
fn amount(value: f64, what: &str) -> Result<f64, ProviderError> {
    if value.is_finite() && value >= 0.0 {
        Ok(value)
    } else {
        Err(ProviderError::Unexpected {
            expected: format!("{what} of zero or more"),
            found: value.to_string(),
        })
    }
}

fn currency(found: &str, expected: &str) -> Result<(), ProviderError> {
    if found.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(ProviderError::Unexpected {
            expected: format!("amounts in {expected}"),
            found: format!("an amount in `{}`", shown(found, 16)),
        })
    }
}

#[derive(Debug, Deserialize)]
struct AnthropicPage {
    data: Vec<AnthropicBucket>,
    has_more: bool,
}

#[derive(Debug, Deserialize)]
struct AnthropicBucket {
    results: Vec<AnthropicResult>,
}

#[derive(Debug, Deserialize)]
struct AnthropicResult {
    amount: String,
    currency: String,
}

/// Anthropic's cost report states each amount as a decimal string in the
/// currency's lowest unit: `"123.45"` in `USD` is $1.2345.
fn anthropic_dollars(body: &str) -> Result<f64, ProviderError> {
    let page: AnthropicPage = read(body)?;
    if page.has_more {
        return Err(ProviderError::Partial);
    }
    let mut cents = 0.0;
    for result in page.data.iter().flat_map(|bucket| &bucket.results) {
        currency(&result.currency, "USD")?;
        let parsed: f64 = result
            .amount
            .trim()
            .parse()
            .map_err(|_| ProviderError::Unexpected {
                expected: "a decimal amount".to_owned(),
                found: format!("`{}`", shown(&result.amount, 32)),
            })?;
        cents += amount(parsed, "a cost")?;
    }
    amount(cents / 100.0, "a total")
}

#[derive(Debug, Deserialize)]
struct OpenaiPage {
    data: Vec<OpenaiBucket>,
    has_more: bool,
}

#[derive(Debug, Deserialize)]
struct OpenaiBucket {
    results: Vec<OpenaiResult>,
}

#[derive(Debug, Deserialize)]
struct OpenaiResult {
    amount: OpenaiAmount,
}

#[derive(Debug, Deserialize)]
struct OpenaiAmount {
    value: f64,
    currency: String,
}

/// OpenAI's costs endpoint states each amount as a number of dollars.
fn openai_dollars(body: &str) -> Result<f64, ProviderError> {
    let page: OpenaiPage = read(body)?;
    if page.has_more {
        return Err(ProviderError::Partial);
    }
    let mut dollars = 0.0;
    for result in page.data.iter().flat_map(|bucket| &bucket.results) {
        currency(&result.amount.currency, "usd")?;
        dollars += amount(result.amount.value, "a cost")?;
    }
    amount(dollars, "a total")
}

#[derive(Debug, Deserialize)]
struct GithubUsage {
    #[serde(rename = "usageItems")]
    usage_items: Vec<GithubItem>,
}

#[derive(Debug, Deserialize)]
struct GithubItem {
    #[serde(rename = "unitType")]
    unit_type: String,
    #[serde(rename = "grossQuantity")]
    gross_quantity: f64,
}

/// GitHub's billing usage reports rows of quantities. The gross quantity is
/// what was drawn, before the plan's included allowance is netted off, which
/// is the figure a pool's `monthly_allowance` is measured against.
fn github_quantity(body: &str, unit: &str) -> Result<f64, ProviderError> {
    let usage: GithubUsage = read(body)?;
    let mut quantity = 0.0;
    for item in &usage.usage_items {
        if !item.unit_type.eq_ignore_ascii_case(unit) {
            return Err(ProviderError::Unexpected {
                expected: format!("rows counted in `{unit}`"),
                found: format!("a row counted in `{}`", shown(&item.unit_type, 32)),
            });
        }
        quantity += amount(item.gross_quantity, "a quantity")?;
    }
    amount(quantity, "a total")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-11-23T12:00:00Z.
    const NOON: u64 = 1_795_435_200;

    fn secret() -> Secret {
        Secret::new("s3cr3t-value".to_owned())
    }

    fn ok(body: &str) -> UsageResponse {
        UsageResponse {
            status: 200,
            body: body.to_owned(),
        }
    }

    fn month_of(provider: UsageProvider, body: &str) -> Result<f64, ProviderError> {
        parse(provider, &ok(body), NOON).map(|windows| {
            assert_eq!(windows.len(), 1, "{windows:?}");
            assert_eq!(windows[0].label, "month");
            assert_eq!(
                windows[0]
                    .resets_at
                    .map(crate::util::rfc3339_utc)
                    .as_deref(),
                Some("2026-12-01T00:00:00Z")
            );
            windows[0].used
        })
    }

    #[test]
    fn a_secret_never_formats_as_itself() {
        let request = request(UsageProvider::OpenaiCost, None, secret(), NOON).expect("a request");
        let printed = format!("{request:?} {:?}", secret());
        assert!(!printed.contains("s3cr3t"), "{printed}");
        assert!(printed.contains("<redacted>"), "{printed}");
        assert_eq!(secret().expose(), "s3cr3t-value");
    }

    #[test]
    fn each_provider_is_asked_at_its_documented_address_with_its_documented_headers() {
        let anthropic =
            request(UsageProvider::AnthropicCost, None, secret(), NOON).expect("a request");
        assert_eq!(
            anthropic.url,
            "https://api.anthropic.com/v1/organizations/cost_report\
             ?starting_at=2026-11-01T00:00:00Z&bucket_width=1d&limit=31"
        );
        assert_eq!(
            anthropic.headers,
            vec![("anthropic-version", "2026-10-21".to_owned())]
        );
        assert_eq!(
            (anthropic.credential.header, anthropic.credential.scheme),
            ("x-api-key", "")
        );

        let openai = request(UsageProvider::OpenaiCost, None, secret(), NOON).expect("a request");
        assert_eq!(
            openai.url,
            "https://api.openai.com/v1/organization/costs\
             ?start_time=1793491200&bucket_width=1d&limit=31"
        );
        assert!(openai.headers.is_empty());
        assert_eq!(
            (openai.credential.header, openai.credential.scheme),
            ("Authorization", "Bearer ")
        );
        assert_eq!(
            crate::util::rfc3339_utc(1_793_491_200),
            "2026-11-01T00:00:00Z"
        );

        for (provider, product) in [
            (UsageProvider::GithubAiCredits, "ai_credit"),
            (UsageProvider::GithubPremiumRequests, "premium_request"),
        ] {
            let github = request(provider, Some("octo-cat"), secret(), NOON).expect("a request");
            assert_eq!(
                github.url,
                format!(
                    "https://api.github.com/users/octo-cat/settings/billing/{product}/usage\
                     ?year=2026&month=11"
                )
            );
            assert_eq!(
                github.headers,
                vec![
                    ("Accept", "application/vnd.github+json".to_owned()),
                    ("X-GitHub-Api-Version", "2026-03-10".to_owned()),
                ]
            );
            assert_eq!(
                (github.credential.header, github.credential.scheme),
                ("Authorization", "Bearer ")
            );
        }
    }

    #[test]
    fn every_request_is_https_to_a_fixed_host_and_carries_no_secret_outside_its_credential() {
        for provider in UsageProvider::ALL {
            let request = request(provider, Some("octo-cat"), secret(), NOON).expect("a request");
            assert!(request.url.starts_with("https://api."), "{}", request.url);
            assert!(!request.url.contains("s3cr3t"), "{}", request.url);
            assert!(
                request.url.chars().all(|c| c.is_ascii_graphic()),
                "an address holds no space, control or non-ASCII character: {}",
                request.url
            );
            for (name, value) in &request.headers {
                assert!(!value.contains("s3cr3t"), "{name}: {value}");
            }
        }
    }

    #[test]
    fn an_account_that_is_not_a_plain_name_never_reaches_an_address() {
        for provider in [
            UsageProvider::GithubAiCredits,
            UsageProvider::GithubPremiumRequests,
        ] {
            for account in [
                None,
                Some(""),
                Some("octo/cat"),
                Some("../orgs/acme"),
                Some("octo cat"),
                Some("octo?per_page=100"),
                Some("octo#x"),
                Some("octo%2Fcat"),
                Some("-octo"),
                Some("octo\ncat"),
            ] {
                assert_eq!(
                    request(provider, account, secret(), NOON),
                    Err(ProviderError::NoAccount { provider }),
                    "{account:?}"
                );
            }
        }
        assert!(
            request(UsageProvider::AnthropicCost, Some("../x"), secret(), NOON).is_ok(),
            "a provider that reads no account does not put one anywhere"
        );
    }

    #[test]
    fn the_month_after_december_is_january_of_the_next_year() {
        assert_eq!(
            crate::util::rfc3339_utc(next_month_start(NOON)),
            "2026-12-01T00:00:00Z"
        );
        let december = next_month_start(NOON);
        assert_eq!(
            crate::util::rfc3339_utc(next_month_start(december)),
            "2027-01-01T00:00:00Z"
        );
        assert_eq!(
            crate::util::rfc3339_utc(next_month_start(december - 1)),
            "2026-12-01T00:00:00Z"
        );
    }

    #[test]
    fn anthropic_cost_is_summed_from_cents_across_every_bucket() {
        let body = r#"{"data":[
            {"starting_at":"2026-11-01T00:00:00Z","ending_at":"2026-11-02T00:00:00Z",
             "results":[{"amount":"123.78912","currency":"USD","cost_type":"tokens"},
                        {"amount":"76.21088","currency":"USD","cost_type":"tokens"}]},
            {"starting_at":"2026-11-02T00:00:00Z","ending_at":"2026-11-03T00:00:00Z",
             "results":[{"amount":"1050","currency":"USD"}]},
            {"starting_at":"2026-11-03T00:00:00Z","ending_at":"2026-11-04T00:00:00Z","results":[]}],
            "has_more":false,"next_page":null}"#;
        let dollars = month_of(UsageProvider::AnthropicCost, body).expect("the documented shape");
        assert!((dollars - 12.5).abs() < 1e-9, "{dollars}");
        assert_eq!(
            month_of(
                UsageProvider::AnthropicCost,
                r#"{"data":[],"has_more":false,"next_page":null}"#
            ),
            Ok(0.0),
            "a month with nothing drawn is zero, stated by the provider"
        );
    }

    #[test]
    fn openai_cost_is_summed_from_dollars_across_every_bucket() {
        let body = r#"{"object":"page","data":[
            {"object":"bucket","start_time":1793491200,"end_time":1793577600,
             "results":[{"object":"organization.costs.result",
                         "amount":{"value":0.06,"currency":"usd"},"line_item":null}]},
            {"object":"bucket","start_time":1793577600,"end_time":1793664000,
             "results":[{"amount":{"value":2,"currency":"USD"}}]}],
            "has_more":false,"next_page":null}"#;
        let dollars = month_of(UsageProvider::OpenaiCost, body).expect("the documented shape");
        assert!((dollars - 2.06).abs() < 1e-9, "{dollars}");
    }

    #[test]
    fn github_usage_is_the_gross_quantity_in_the_products_own_unit() {
        let credits = r#"{"timePeriod":{"year":2026,"month":11},"user":"octo-cat","usageItems":[
            {"product":"Copilot AI Credits","sku":"AI Credit","unitType":"ai-credits",
             "pricePerUnit":0.01,"grossQuantity":1200,"grossAmount":12.0,"discountQuantity":1000,
             "discountAmount":10.0,"netQuantity":200,"netAmount":2.0},
            {"product":"Copilot AI Credits","sku":"AI Credit","unitType":"ai-credits",
             "pricePerUnit":0.01,"grossQuantity":300.5,"grossAmount":3.005,"discountQuantity":300.5,
             "discountAmount":3.005,"netQuantity":0,"netAmount":0.0}]}"#;
        assert_eq!(
            month_of(UsageProvider::GithubAiCredits, credits),
            Ok(1500.5),
            "what was drawn, before the included allowance is netted off"
        );

        let requests = r#"{"timePeriod":{"year":2026},"user":"octo-cat","usageItems":[
            {"product":"Copilot","sku":"Copilot Premium Request","model":"GPT-5",
             "unitType":"requests","pricePerUnit":0.04,"grossQuantity":100,"grossAmount":4.0,
             "discountQuantity":0,"discountAmount":0.0,"netQuantity":100,"netAmount":4.0}]}"#;
        assert_eq!(
            month_of(UsageProvider::GithubPremiumRequests, requests),
            Ok(100.0)
        );
        assert_eq!(
            month_of(UsageProvider::GithubAiCredits, r#"{"usageItems":[]}"#),
            Ok(0.0)
        );
    }

    #[test]
    fn a_page_that_says_more_follow_is_refused_rather_than_read_as_the_month() {
        assert_eq!(
            month_of(
                UsageProvider::AnthropicCost,
                r#"{"data":[{"results":[{"amount":"100","currency":"USD"}]}],
                    "has_more":true,"next_page":"page_x"}"#
            ),
            Err(ProviderError::Partial)
        );
        assert_eq!(
            month_of(
                UsageProvider::OpenaiCost,
                r#"{"data":[{"results":[{"amount":{"value":1.0,"currency":"usd"}}]}],
                    "has_more":true,"next_page":"page_x"}"#
            ),
            Err(ProviderError::Partial)
        );
    }

    #[test]
    fn an_answer_that_is_not_the_documented_one_is_refused_and_never_read_as_zero() {
        let cases: &[(UsageProvider, &str)] = &[
            (UsageProvider::AnthropicCost, ""),
            (UsageProvider::AnthropicCost, "<html>gateway</html>"),
            (UsageProvider::AnthropicCost, "{}"),
            (UsageProvider::AnthropicCost, r#"{"data":[]}"#),
            (
                UsageProvider::AnthropicCost,
                r#"{"data":null,"has_more":false}"#,
            ),
            (
                UsageProvider::AnthropicCost,
                r#"{"data":[{"results":[{"amount":12.5,"currency":"USD"}]}],"has_more":false}"#,
            ),
            (UsageProvider::OpenaiCost, r#"{"has_more":false}"#),
            (
                UsageProvider::OpenaiCost,
                r#"{"data":[{"results":[{"amount":{"value":"1.0","currency":"usd"}}]}],
                    "has_more":false}"#,
            ),
            (
                UsageProvider::OpenaiCost,
                r#"{"data":[{"results":[{"amount":{"currency":"usd"}}]}],"has_more":false}"#,
            ),
            (UsageProvider::GithubAiCredits, "{}"),
            (UsageProvider::GithubAiCredits, r#"{"usageItems":null}"#),
            (
                UsageProvider::GithubAiCredits,
                r#"{"usageItems":[{"unitType":"ai-credits"}]}"#,
            ),
            (
                UsageProvider::GithubPremiumRequests,
                r#"{"usageItems":[{"grossQuantity":3}]}"#,
            ),
        ];
        for (provider, body) in cases {
            assert!(
                matches!(
                    month_of(*provider, body),
                    Err(ProviderError::Unreadable { .. })
                ),
                "{provider} given {body:?} answered {:?}",
                month_of(*provider, body)
            );
        }
    }

    #[test]
    fn a_currency_a_unit_or_a_number_nobody_expects_is_refused() {
        let cases: &[(UsageProvider, &str)] = &[
            (
                UsageProvider::AnthropicCost,
                r#"{"data":[{"results":[{"amount":"100","currency":"EUR"}]}],"has_more":false}"#,
            ),
            (
                UsageProvider::AnthropicCost,
                r#"{"data":[{"results":[{"amount":"-100","currency":"USD"}]}],"has_more":false}"#,
            ),
            (
                UsageProvider::AnthropicCost,
                r#"{"data":[{"results":[{"amount":"1e999","currency":"USD"}]}],"has_more":false}"#,
            ),
            (
                UsageProvider::AnthropicCost,
                r#"{"data":[{"results":[{"amount":"twelve","currency":"USD"}]}],"has_more":false}"#,
            ),
            (
                UsageProvider::AnthropicCost,
                r#"{"data":[{"results":[{"amount":"NaN","currency":"USD"}]}],"has_more":false}"#,
            ),
            (
                UsageProvider::OpenaiCost,
                r#"{"data":[{"results":[{"amount":{"value":1.0,"currency":"gbp"}}]}],
                    "has_more":false}"#,
            ),
            (
                UsageProvider::OpenaiCost,
                r#"{"data":[{"results":[{"amount":{"value":-0.5,"currency":"usd"}}]}],
                    "has_more":false}"#,
            ),
            (
                UsageProvider::GithubAiCredits,
                r#"{"usageItems":[{"unitType":"requests","grossQuantity":5}]}"#,
            ),
            (
                UsageProvider::GithubPremiumRequests,
                r#"{"usageItems":[{"unitType":"ai-credits","grossQuantity":5}]}"#,
            ),
            (
                UsageProvider::GithubPremiumRequests,
                r#"{"usageItems":[{"unitType":"requests","grossQuantity":-5}]}"#,
            ),
        ];
        for (provider, body) in cases {
            assert!(
                matches!(
                    month_of(*provider, body),
                    Err(ProviderError::Unexpected { .. })
                ),
                "{provider} given {body:?} answered {:?}",
                month_of(*provider, body)
            );
        }
    }

    #[test]
    fn every_status_but_200_is_an_error_that_says_which_and_never_reads_the_body() {
        let a_valid_body = r#"{"data":[],"has_more":false}"#;
        for (status, expected) in [
            (401, ProviderError::CredentialRefused { status: 401 }),
            (403, ProviderError::CredentialRefused { status: 403 }),
            (404, ProviderError::NotFound),
            (429, ProviderError::RateLimited),
            (500, ProviderError::Status { status: 500 }),
            (204, ProviderError::Status { status: 204 }),
            (301, ProviderError::Status { status: 301 }),
            (0, ProviderError::Status { status: 0 }),
        ] {
            assert_eq!(
                parse(
                    UsageProvider::AnthropicCost,
                    &UsageResponse {
                        status,
                        body: a_valid_body.to_owned(),
                    },
                    NOON
                ),
                Err(expected),
                "HTTP {status}"
            );
        }
    }

    #[test]
    fn an_error_never_repeats_a_long_or_hostile_value_from_the_answer() {
        let hostile = "\\u001b[2J".repeat(40);
        let printable = |error: &ProviderError| {
            error
                .to_string()
                .chars()
                .all(|c| c == ' ' || c == '…' || c.is_ascii_graphic())
        };

        let odd_currency = format!(
            r#"{{"data":[{{"results":[{{"amount":"1","currency":"{hostile}"}}]}}],"has_more":false}}"#
        );
        let error =
            month_of(UsageProvider::AnthropicCost, &odd_currency).expect_err("an odd currency");
        assert!(
            matches!(error, ProviderError::Unexpected { .. }),
            "{error:?}"
        );
        assert!(printable(&error), "{error:?}");
        assert!(error.to_string().chars().count() < 120, "{error}");

        let odd_amount = format!(
            r#"{{"data":[{{"results":[{{"amount":"{hostile}","currency":"USD"}}]}}],"has_more":false}}"#
        );
        let error = month_of(UsageProvider::AnthropicCost, &odd_amount).expect_err("an odd amount");
        assert!(printable(&error), "{error:?}");

        let wrong_type = format!(r#"{{"data":"{hostile}","has_more":false}}"#);
        let error = month_of(UsageProvider::AnthropicCost, &wrong_type).expect_err("a wrong type");
        assert!(
            matches!(error, ProviderError::Unreadable { .. }),
            "{error:?}"
        );
        assert!(printable(&error), "{error:?}");
        assert!(error.to_string().chars().count() < 320, "{error}");

        assert_eq!(shown("ok", 16), "ok");
        assert_eq!(shown("abcdef", 3), "abc…");
        assert_eq!(shown("a\u{1b}b\nc\u{e9}", 16), "a?b?c?");
    }
}
