//! The tracker's one way out of the machine: `curl`, as a subprocess, through
//! the Runner (DESIGN.md §4.2, §13).
//!
//! upstroke links no HTTP client. The engine never speaks HTTP, and the
//! tracker, which may, does it the way the engine does everything else that
//! reaches outside: it starts a separately installed tool and reads what the
//! tool prints. So the binary holds no TLS stack and no socket code, and the
//! tool's own proxy and certificate handling is the platform's.
//!
//! The standard's six questions for a subprocess integration
//! (CODING_STANDARDS.md §9), answered here:
//!
//! - **Discovery.** `curl` is resolved on `PATH` by the Runner, as every
//!   program is. An absent `curl` is [`TransportError::NotStarted`], carrying
//!   the Runner's own account of the lookup.
//! - **Directory and environment.** The Runner's, as for a probe: the
//!   process's environment, so the operator's proxy settings apply. `-q` is
//!   the first argument, so no `.curlrc` is read.
//! - **Timeout and cleanup.** `curl` is told to give up after
//!   [`TRANSFER_TIMEOUT`]; the Runner kills its process tree after
//!   [`PROCESS_TIMEOUT`] if it has not.
//! - **Exit status and output.** Both, since neither alone is sufficient:
//!   exit 0 with a body and a status line it printed, or an error. `curl` is
//!   not asked to fail on an HTTP error, so a 401 arrives as a status to
//!   classify rather than as exit 22 and nothing else.
//! - **Size and encoding.** The Runner bounds what it captures, `curl` is
//!   told the body's limit, and a body over [`MAX_BODY_BYTES`] is refused
//!   here whatever either did.
//! - **Secrets.** The credential is written to `curl`'s standard input as a
//!   config line and appears in no argument, no environment variable this
//!   module sets, and no error.

#![forbid(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use crate::runner::{CommandSpec, Runner};

use super::provider::{UsageRequest, UsageResponse};

/// The program. A bare name: the Runner resolves it.
pub const CURL: &str = "curl";

/// How long `curl` may take to connect.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// How long `curl` may take over the whole transfer before it gives up.
pub const TRANSFER_TIMEOUT: Duration = Duration::from_secs(30);

/// How long the Runner lets the process live. Longer than
/// [`TRANSFER_TIMEOUT`], so a healthy `curl` reports its own timeout and only
/// a wedged one is killed.
pub const PROCESS_TIMEOUT: Duration = Duration::from_secs(45);

/// The largest body a usage endpoint may answer with. A month of daily cost
/// buckets is a few kilobytes.
pub const MAX_BODY_BYTES: usize = 1024 * 1024;

/// Why a request produced no response.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TransportError {
    #[error("the request cannot be sent as written: {0}")]
    Unsendable(&'static str),
    #[error("`curl` could not be started ({0}); the tracker asks provider endpoints through it")]
    NotStarted(String),
    #[error("`curl` did not finish within {} seconds and was stopped", PROCESS_TIMEOUT.as_secs())]
    TimedOut,
    #[error("`curl` exited {code}: {stderr}")]
    Failed { code: String, stderr: String },
    #[error("the answer was larger than the {MAX_BODY_BYTES} bytes a usage report can be")]
    TooLarge,
    #[error("`curl` printed no status line after the body, so the answer cannot be classified")]
    NoStatus,
}

/// One HTTPS `GET`, answered or not. The seam the tracker is tested through:
/// production hands it [`CurlTransport`], a test hands it a table.
pub trait UsageTransport {
    /// # Errors
    ///
    /// [`TransportError`] when no HTTP response was obtained. A response with
    /// an error status is a response, and is returned as one.
    fn get(&self, request: &UsageRequest) -> Result<UsageResponse, TransportError>;
}

/// [`UsageTransport`] over a `curl` subprocess the Runner starts.
pub struct CurlTransport<'a> {
    runner: &'a dyn Runner,
    asked: AtomicU32,
}

impl<'a> CurlTransport<'a> {
    #[must_use]
    pub fn new(runner: &'a dyn Runner) -> Self {
        Self {
            runner,
            asked: AtomicU32::new(0),
        }
    }
}

/// The arguments every request is made with. No address and no header: those
/// go on standard input, where a process listing cannot read them.
#[must_use]
pub fn arguments() -> Vec<String> {
    [
        "-q",
        "--silent",
        "--show-error",
        "--proto",
        "=https",
        "--tlsv1.2",
        "--connect-timeout",
        &CONNECT_TIMEOUT.as_secs().to_string(),
        "--max-time",
        &TRANSFER_TIMEOUT.as_secs().to_string(),
        "--max-filesize",
        &MAX_BODY_BYTES.to_string(),
        "--write-out",
        "\\n%{http_code}",
        "--config",
        "-",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

/// The request as `curl` config lines: the address, then one `header` line
/// per header, the credential's last.
///
/// # Errors
///
/// [`TransportError::Unsendable`] for a value `curl`'s config syntax cannot
/// carry on one line. A config file is one option per line, so a newline in a
/// value would end the line and begin an option of the value's choosing; a
/// control character anywhere is refused rather than escaped, and nothing
/// about the refused value is repeated, because it may be the credential.
pub fn config(request: &UsageRequest) -> Result<String, TransportError> {
    if !request.url.starts_with("https://") {
        return Err(TransportError::Unsendable(
            "the address is not an https:// address",
        ));
    }
    let secret = request.credential.secret.expose();
    if secret.is_empty() {
        return Err(TransportError::Unsendable("the credential is empty"));
    }
    let mut lines = vec![format!("url = {}", quoted(&request.url, "the address")?)];
    for (name, value) in &request.headers {
        lines.push(header_line(name, value, "a header")?);
    }
    lines.push(header_line(
        request.credential.header,
        &format!("{}{secret}", request.credential.scheme),
        "the credential",
    )?);
    let mut text = lines.join("\n");
    text.push('\n');
    Ok(text)
}

fn header_line(name: &str, value: &str, what: &'static str) -> Result<String, TransportError> {
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(TransportError::Unsendable("a header name is not a token"));
    }
    Ok(format!(
        "header = {}",
        quoted(&format!("{name}: {value}"), what)?
    ))
}

/// `text` as a double-quoted `curl` config value: `\` and `"` escaped, and
/// anything a line cannot hold refused.
fn quoted(text: &str, what: &'static str) -> Result<String, TransportError> {
    if text.chars().any(char::is_control) {
        return Err(TransportError::Unsendable(match what {
            "the credential" => {
                "the credential holds a control character, such as a line break; a key is one \
                 line of printable text"
            }
            "the address" => "the address holds a control character",
            _ => "a header holds a control character",
        }));
    }
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for character in text.chars() {
        if matches!(character, '\\' | '"') {
            out.push('\\');
        }
        out.push(character);
    }
    out.push('"');
    Ok(out)
}

/// What `curl` printed, as a response: everything before the last line is the
/// body, and the last line is the status `--write-out` appended.
///
/// # Errors
///
/// [`TransportError::NoStatus`] when the last line is not a three-digit
/// status, and [`TransportError::TooLarge`] for a body over the bound.
pub fn response(stdout: &str) -> Result<UsageResponse, TransportError> {
    let (body, status) = stdout.rsplit_once('\n').ok_or(TransportError::NoStatus)?;
    if status.len() != 3 || !status.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(TransportError::NoStatus);
    }
    let status: u16 = status.parse().map_err(|_| TransportError::NoStatus)?;
    if body.len() > MAX_BODY_BYTES {
        return Err(TransportError::TooLarge);
    }
    Ok(UsageResponse {
        status,
        body: body.to_owned(),
    })
}

/// Text `curl` or the Runner produced, made fit for one terminal line.
fn one_line(text: &str, max: usize) -> String {
    let mut out: String = text
        .trim()
        .chars()
        .take(max)
        .map(|c| {
            if c == ' ' || c.is_ascii_graphic() {
                c
            } else {
                ' '
            }
        })
        .collect();
    if text.trim().chars().nth(max).is_some() {
        out.push('…');
    }
    out
}

impl UsageTransport for CurlTransport<'_> {
    fn get(&self, request: &UsageRequest) -> Result<UsageResponse, TransportError> {
        let stdin = config(request)?;
        let mut command = CommandSpec::new(CURL);
        for argument in arguments() {
            command = command.arg(argument);
        }
        let ordinal = self.asked.fetch_add(1, Ordering::Relaxed);
        let request =
            crate::agent::probe_request(CURL, command.stdin(stdin), ordinal, PROCESS_TIMEOUT)
                .map_err(|error| TransportError::NotStarted(one_line(&error.to_string(), 300)))?;
        let output = self
            .runner
            .run_blocking(&request)
            .map_err(|error| TransportError::NotStarted(one_line(&error.to_string(), 300)))?;
        if output.timed_out {
            return Err(TransportError::TimedOut);
        }
        if output.output_limited {
            return Err(TransportError::TooLarge);
        }
        if output.code != Some(0) {
            return Err(TransportError::Failed {
                code: output
                    .code
                    .map_or_else(|| "by a signal".to_owned(), |code| code.to_string()),
                stderr: one_line(&output.stderr, 300),
            });
        }
        response(&output.stdout)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::super::provider::{Credential, Secret};
    use super::*;
    use crate::agent::ProcessOutput;
    use crate::runner::{RunFuture, RunnerCall, RunnerError, RunnerRequest};

    fn request(secret: &str) -> UsageRequest {
        UsageRequest {
            url: "https://api.example.test/v1/usage?starting_at=2026-11-01T00:00:00Z&limit=31"
                .to_owned(),
            headers: vec![("anthropic-version", "2026-10-21".to_owned())],
            credential: Credential {
                header: "x-api-key",
                scheme: "",
                secret: Secret::new(secret.to_owned()),
            },
        }
    }

    /// A Runner that records what it was asked to run and answers from a
    /// table, so the transport is driven with no process and no network.
    struct Scripted {
        answer: Result<ProcessOutput, String>,
        seen: Mutex<Vec<RunnerRequest>>,
    }

    impl Scripted {
        fn answering(code: Option<i32>, stdout: &str, stderr: &str) -> Self {
            Self {
                answer: Ok(ProcessOutput {
                    code,
                    stdout: stdout.to_owned(),
                    stderr: stderr.to_owned(),
                    duration: Duration::ZERO,
                    timed_out: false,
                    output_limited: false,
                }),
                seen: Mutex::new(Vec::new()),
            }
        }

        fn seen(&self) -> Vec<RunnerRequest> {
            self.seen
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone()
        }
    }

    impl Runner for Scripted {
        fn run<'a>(&'a self, request: &'a RunnerRequest, _call: RunnerCall<'a>) -> RunFuture<'a> {
            self.seen
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(request.clone());
            let answer = match &self.answer {
                Ok(output) => Ok(output.clone()),
                Err(message) => Err(RunnerError::never_started(
                    &request.invocation,
                    crate::error::UpstrokeError::Refused {
                        message: message.clone(),
                    },
                )),
            };
            Box::pin(async move { answer })
        }
    }

    #[test]
    fn the_request_goes_to_curl_on_standard_input_and_the_secret_is_in_no_argument() {
        let runner = Scripted::answering(Some(0), "{\"data\":[]}\n200", "");
        let transport = CurlTransport::new(&runner);
        let answer = transport
            .get(&request("sk-admin-s3cr3t"))
            .expect("curl answered");
        assert_eq!(
            answer,
            UsageResponse {
                status: 200,
                body: "{\"data\":[]}".to_owned(),
            }
        );

        let seen = runner.seen();
        assert_eq!(seen.len(), 1);
        let ran = &seen[0];
        assert_eq!(ran.command.program, "curl");
        assert_eq!(ran.command.args, arguments());
        assert_eq!(ran.timeout, PROCESS_TIMEOUT);
        assert!(
            ran.command.env.is_empty(),
            "nothing is added to the environment"
        );
        for argument in &ran.command.args {
            assert!(!argument.contains("s3cr3t"), "{argument}");
            assert!(!argument.contains("example.test"), "{argument}");
        }
        assert_eq!(
            String::from_utf8(ran.command.stdin.clone()).expect("the config is text"),
            "url = \"https://api.example.test/v1/usage?starting_at=2026-11-01T00:00:00Z&limit=31\"\n\
             header = \"anthropic-version: 2026-10-21\"\n\
             header = \"x-api-key: sk-admin-s3cr3t\"\n"
        );
    }

    #[test]
    fn reset_request_is_exactly_one_bounded_get_with_only_the_authorized_headers() {
        let runner = Scripted::answering(Some(0), "{}\n200", "");
        let transport = CurlTransport::new(&runner);
        let request =
            super::super::reset::request(Secret::new("synthetic-reset-secret".to_owned()));
        transport.get(&request).expect("the fake runner answers");
        let seen = runner.seen();
        assert_eq!(seen.len(), 1);
        let ran = seen.first().expect("one request");
        assert_eq!(ran.command.program, "curl");
        assert_eq!(ran.command.args, arguments());
        assert_eq!(ran.timeout, PROCESS_TIMEOUT);
        assert!(ran.command.env.is_empty());
        assert_eq!(
            std::str::from_utf8(&ran.command.stdin).expect("text config"),
            "url = \"https://api.anthropic.com/api/oauth/usage?cedar_ember=1&skip_spend=1\"\n\
             header = \"anthropic-beta: oauth-2025-04-20\"\n\
             header = \"Authorization: Bearer synthetic-reset-secret\"\n"
        );
        for forbidden in [
            "--request",
            "-X",
            "--data",
            "--data-raw",
            "--location",
            "-L",
            "--user-agent",
        ] {
            assert!(!ran.command.args.iter().any(|arg| arg == forbidden));
        }
        assert!(!format!("{:?}", ran.command).contains("synthetic-reset-secret"));
    }

    #[test]
    fn the_arguments_disable_curlrc_first_and_allow_nothing_but_https() {
        let arguments = arguments();
        assert_eq!(arguments.first().map(String::as_str), Some("-q"));
        let follows = |flag: &str| {
            arguments
                .iter()
                .position(|argument| argument == flag)
                .and_then(|at| arguments.get(at + 1))
                .map(String::as_str)
        };
        assert_eq!(follows("--proto"), Some("=https"));
        assert_eq!(follows("--config"), Some("-"));
        assert_eq!(follows("--write-out"), Some("\\n%{http_code}"));
        assert_eq!(follows("--max-time"), Some("30"));
        assert_eq!(follows("--connect-timeout"), Some("10"));
        assert_eq!(follows("--max-filesize"), Some("1048576"));
        for forbidden in [
            "-L",
            "--location",
            "--location-trusted",
            "-k",
            "--insecure",
            "-H",
        ] {
            assert!(
                !arguments.iter().any(|argument| argument == forbidden),
                "`{forbidden}` follows redirects, drops verification or puts a header on the \
                 command line"
            );
        }
        assert!(TRANSFER_TIMEOUT < PROCESS_TIMEOUT);
    }

    #[test]
    fn a_quote_or_a_backslash_in_a_value_is_escaped_and_cannot_end_the_value() {
        let config = config(&request(r#"a"b\c"#)).expect("printable text is sendable");
        assert!(
            config.ends_with("header = \"x-api-key: a\\\"b\\\\c\"\n"),
            "{config}"
        );
        assert_eq!(config.lines().count(), 3, "{config}");
    }

    #[test]
    fn a_value_that_could_start_a_second_config_line_is_refused_and_not_repeated() {
        for hostile in [
            "zq9s3cr3t\noutput = /tmp/x",
            "zq9s3cr3t\r\nurl = \"https://elsewhere.test\"",
            "zq9s3cr3t\u{0}",
            "zq9s3cr3t\tvalue",
            "zq9s3cr3t\u{1b}[2J",
        ] {
            let error = config(&request(hostile)).expect_err("one option per line");
            assert!(matches!(error, TransportError::Unsendable(_)), "{error:?}");
            let said = error.to_string();
            assert!(!said.contains("zq9s3cr3t"), "{said}");
            assert!(said.chars().all(|c| !c.is_control()), "{said:?}");
        }
        assert_eq!(
            config(&request("")),
            Err(TransportError::Unsendable("the credential is empty"))
        );

        let mut plain_http = request("key");
        plain_http.url = "http://api.example.test/v1/usage".to_owned();
        assert!(matches!(
            config(&plain_http),
            Err(TransportError::Unsendable(_))
        ));

        let mut split_header = request("key");
        split_header.headers = vec![("accept", "a\nheader = \"x: y\"".to_owned())];
        assert!(matches!(
            config(&split_header),
            Err(TransportError::Unsendable(_))
        ));

        let mut odd_name = request("key");
        odd_name.headers = vec![("x y", "v".to_owned())];
        assert!(matches!(
            config(&odd_name),
            Err(TransportError::Unsendable(_))
        ));
    }

    #[test]
    fn a_refused_request_starts_no_process() {
        let runner = Scripted::answering(Some(0), "\n200", "");
        let transport = CurlTransport::new(&runner);
        assert!(transport.get(&request("key\nurl = x")).is_err());
        assert!(runner.seen().is_empty());
    }

    #[test]
    fn the_status_is_the_last_line_and_the_body_is_everything_before_it() {
        assert_eq!(
            response("{\"a\":1}\n200"),
            Ok(UsageResponse {
                status: 200,
                body: "{\"a\":1}".to_owned(),
            })
        );
        assert_eq!(
            response("line one\nline two\n\n401"),
            Ok(UsageResponse {
                status: 401,
                body: "line one\nline two\n".to_owned(),
            })
        );
        assert_eq!(
            response("\n000"),
            Ok(UsageResponse {
                status: 0,
                body: String::new(),
            }),
            "curl prints 000 when it got no response; the status is classified, not trusted"
        );
        for unreadable in [
            "",
            "200",
            "{\"a\":1}",
            "body\n20",
            "body\n2000",
            "body\n2x0",
            "body\n",
        ] {
            assert_eq!(
                response(unreadable),
                Err(TransportError::NoStatus),
                "{unreadable:?}"
            );
        }
        let large = format!("{}\n200", "x".repeat(MAX_BODY_BYTES + 1));
        assert_eq!(response(&large), Err(TransportError::TooLarge));
    }

    #[test]
    fn a_curl_that_fails_times_out_or_cannot_start_is_an_error_that_says_which() {
        let failed = Scripted::answering(
            Some(6),
            "\n000",
            "curl: (6) Could not resolve host: api.example.test\n",
        );
        assert_eq!(
            CurlTransport::new(&failed).get(&request("key")),
            Err(TransportError::Failed {
                code: "6".to_owned(),
                stderr: "curl: (6) Could not resolve host: api.example.test".to_owned(),
            })
        );

        let killed = Scripted::answering(None, "", "");
        assert!(matches!(
            CurlTransport::new(&killed).get(&request("key")),
            Err(TransportError::Failed { code, .. }) if code == "by a signal"
        ));

        let mut wedged = Scripted::answering(None, "", "");
        if let Ok(output) = &mut wedged.answer {
            output.timed_out = true;
        }
        assert_eq!(
            CurlTransport::new(&wedged).get(&request("key")),
            Err(TransportError::TimedOut)
        );

        let mut flooding = Scripted::answering(None, "", "");
        if let Ok(output) = &mut flooding.answer {
            output.output_limited = true;
        }
        assert_eq!(
            CurlTransport::new(&flooding).get(&request("key")),
            Err(TransportError::TooLarge)
        );

        let absent = Scripted {
            answer: Err("`curl` was not found on PATH".to_owned()),
            seen: Mutex::new(Vec::new()),
        };
        let error = CurlTransport::new(&absent)
            .get(&request("key"))
            .expect_err("no curl, no answer");
        assert!(matches!(error, TransportError::NotStarted(_)), "{error:?}");
        assert!(error.to_string().contains("not found on PATH"), "{error}");
    }

    #[test]
    fn each_request_is_its_own_invocation() {
        let runner = Scripted::answering(Some(0), "\n200", "");
        let transport = CurlTransport::new(&runner);
        for _ in 0..3 {
            transport.get(&request("key")).expect("curl answered");
        }
        let ids: Vec<String> = runner
            .seen()
            .iter()
            .map(|request| request.invocation.render())
            .collect();
        assert_eq!(
            ids,
            vec!["p.agent-curl.o0", "p.agent-curl.o1", "p.agent-curl.o2"]
        );
    }

    #[test]
    fn what_curl_wrote_to_stderr_is_one_bounded_printable_line() {
        assert_eq!(one_line("  a\nb\u{1b}[2Jc  ", 300), "a b [2Jc");
        assert_eq!(one_line(&"x".repeat(400), 300).chars().count(), 301);
    }
}
