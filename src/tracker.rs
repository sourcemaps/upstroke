//! The capacity tracker: the one part of upstroke that speaks HTTP, and the
//! reason it is not the engine (DESIGN.md §4.2, §13).
//!
//! **What it is.** Opt-in reads: `capacity --refresh` for billing and
//! `capacity --reset-credits claude` for advisory voluntary reset inventory.
//! The latter needs no pool, reads only an explicitly provider-authorized
//! operator bearer, and stores nothing. `capacity --subscription-usage claude`
//! reads independent windows and grants through the CLI's own login or that
//! same bearer GET, with no fallback or persistence. For each billing pool that names a
//! `usage_provider`, it asks that provider's documented usage endpoint how
//! much the account has drawn, and stores the answer under the machine
//! directory (`~/.upstroke/capacity/usage/`). Nothing else: it dispatches no
//! agent, runs no gate, owns no git and writes no event log.
//!
//! **What it is not.** The engine. `run` and `resume` never call this module,
//! never start its transport, and never wait on a provider: they read the
//! files it left, through `capacity::machine`, as they read any other file.
//! That direction is the invariant. A run's behaviour is a function of its
//! event log and the files on disk, and a network answer that arrived or did
//! not arrive mid-run is neither.
//!
//! **How the split is held.** [`refresh`], [`reset_credits`] and [`subscription_usage`] are on
//! `clippy.toml`'s denylist as effectful wrappers, so a module that states no
//! allowance for governed lints cannot name them, and every topology module
//! forbids them. The frozen legacy
//! modules do carry an allowance, so the denylist alone does not cover them:
//! `tests::only_the_cli_reaches_the_tracker` reads every production source
//! file and fails if anything but `src/main.rs` names this module.
//!
//! **Its one effect on disk** is [`publish`]: one report file per pool, staged
//! beside its destination under a name no other writer can pick and renamed
//! into place, so a reader sees the old report or the new one and never part
//! of either. The write is atomic and not durable: a crash can lose the newest
//! report, and the next refresh writes it again. A failed refresh leaves the
//! stored report alone, since an older reading is still a valid ceiling until
//! its window resets (`capacity::machine::reading`). That holds of one source:
//! each report records the name of the variable its credential was read from
//! (`capacity::usage::token_env_for`) and, where the provider reports per
//! account, the account it was asked about (`capacity::usage::account_for`),
//! and once the pool names another of either, the old report bounds nothing.
//! It records names only, never the credential.

// Allowlist placement: the funnel section of `effects/allowlist.toml`, which
// carries this module's review clause.
#![allow(clippy::disallowed_methods)]
#![forbid(clippy::disallowed_types, clippy::disallowed_macros)]

use std::path::{Path, PathBuf};

use crate::capacity::usage::{self, UsageReport, UsageWindow};
use crate::capacity::{MAX_TOKEN_ENV_CHARS, Pool, UsageProvider};
use crate::error::UpstrokeError;

pub mod provider;
pub mod reset;
pub mod subscription;
pub mod transport;

pub use self::provider::Secret;
pub use self::transport::{CurlTransport, UsageTransport};

/// Read Claude's voluntary reset inventory, independently of billing pools.
/// Only `reset::TOKEN_ENV` is resolved; the caller supplies the environment
/// read and transport. Nothing is stored. Missing or invalid credentials
/// start no request. Failures are advisory and contain no provider text.
/// `now_unix` dates the observation; `None` reads the clock after the response.
pub fn reset_credits(
    transport: &dyn UsageTransport,
    credential: &dyn Fn(&str) -> Option<String>,
    now_unix: Option<u64>,
) -> reset::ResetReport {
    match reset_response(transport, credential) {
        Ok(response) => reset::parse(
            &response,
            now_unix.unwrap_or_else(crate::util::unix_seconds_now),
        ),
        Err(reason) => reset::ResetReport::Unavailable(reason),
    }
}

fn reset_response(
    transport: &dyn UsageTransport,
    credential: &dyn Fn(&str) -> Option<String>,
) -> Result<provider::UsageResponse, reset::Unavailable> {
    use reset::Unavailable;

    let Some(token) = credential(reset::TOKEN_ENV).filter(|value| !value.is_empty()) else {
        return Err(Unavailable::MissingCredential);
    };
    if !token.bytes().all(|byte| byte.is_ascii_graphic()) {
        return Err(Unavailable::InvalidCredential);
    }
    let request = reset::request(Secret::new(token));
    // Never expose process output, including errors that echo the credential.
    transport
        .get(&request)
        .map_err(|error| Unavailable::from_transport(&error))
}

/// Read advisory subscription windows and grants through exactly one selected
/// route. CLI auth stays inside Claude; bearer auth resolves only TOKEN_ENV.
/// No fallback, persistence, redemption or pool arithmetic occurs here.
pub fn subscription_usage(
    auth: subscription::Auth,
    runner: &dyn crate::runner::Runner,
    transport: &dyn UsageTransport,
    credential: &dyn Fn(&str) -> Option<String>,
    now_unix: Option<u64>,
) -> subscription::Report {
    let payload = match auth {
        subscription::Auth::Cli => subscription::read_cli(runner),
        subscription::Auth::Bearer => reset_response(transport, credential)
            .and_then(|response| reset::response_value(&response)),
    };
    match payload {
        Ok(value) => subscription::parse(
            &value,
            auth,
            now_unix.unwrap_or_else(crate::util::unix_seconds_now),
        ),
        Err(reason) => subscription::Report::Unavailable(reason),
    }
}

/// What to refresh, found the way `upstroke capacity` finds it.
#[derive(Debug, Clone, Default)]
pub struct RefreshOptions {
    pub config_path: Option<PathBuf>,
    pub pools_path: Option<PathBuf>,
    pub repo_root: PathBuf,
    /// The moment the reports are dated. `None` reads the clock.
    pub now_unix: Option<u64>,
}

/// What became of one pool's refresh.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// The provider answered and the report is stored.
    Stored(Vec<UsageWindow>),
    /// Nothing was asked, and why. The stored report, if any, is untouched.
    Skipped(String),
    /// The provider was asked and no report came of it, and why. The stored
    /// report, if any, is untouched.
    Failed(String),
}

/// One pool's refresh.
#[derive(Debug, Clone, PartialEq)]
pub struct PoolRefresh {
    pub pool: String,
    pub provider: UsageProvider,
    pub outcome: Outcome,
}

/// Every pool that names a provider, and what became of it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RefreshReport {
    pub pools: Vec<PoolRefresh>,
}

/// Ask every pool's usage provider and store what each answers.
///
/// `credential` resolves the name a pool gives in `usage_token_env` to that
/// variable's value. It is a parameter so that the one read of the process
/// environment is the caller's, at the CLI edge, and a test supplies a table.
///
/// One pool's failure is that pool's [`Outcome`] and never ends the refresh:
/// a provider that is down must not cost the reports of the ones that are up.
///
/// # Errors
///
/// [`UpstrokeError`] when the configuration cannot be loaded, or when there
/// is no machine directory to store a report in. A provider's refusal, a
/// missing credential and a failed write are outcomes, not errors.
pub fn refresh(
    opts: &RefreshOptions,
    transport: &dyn UsageTransport,
    credential: &dyn Fn(&str) -> Option<String>,
) -> Result<RefreshReport, UpstrokeError> {
    // The configuration's warnings are not this command's to print: the CLI
    // asks `capacity::report` next, which loads the same files and reports
    // every one of them.
    // It takes the report's reading (`config::load_for_report`) although it
    // uses only the pools file, because the report follows it in the same
    // command: a file the report would refuse stops the command here, before
    // any provider is asked or anything is stored.
    let mut unreported = Vec::new();
    let config = crate::config::load_for_report(
        opts.config_path.as_deref(),
        &opts.repo_root,
        opts.pools_path.as_deref(),
        &mut unreported,
    )?;
    let Some(machine_dir) = config.machine_dir.as_deref() else {
        return Err(UpstrokeError::Refused {
            message: "no home directory resolves, so there is no machine directory to store a \
                      provider report in — pass --pools <path> to name the pools file, and its \
                      directory is used"
                .to_owned(),
        });
    };
    let store = usage::store_dir(machine_dir);
    let now_unix = opts.now_unix.unwrap_or_else(crate::util::unix_seconds_now);
    Ok(RefreshReport {
        pools: config
            .pools
            .into_iter()
            .filter_map(|pool| {
                let provider = pool.usage.as_ref()?.provider;
                let outcome = refresh_pool(&pool, &store, transport, credential, now_unix);
                Some(PoolRefresh {
                    pool: pool.name,
                    provider,
                    outcome,
                })
            })
            .collect(),
    })
}

fn refresh_pool(
    pool: &Pool,
    store: &Path,
    transport: &dyn UsageTransport,
    credential: &dyn Fn(&str) -> Option<String>,
    now_unix: u64,
) -> Outcome {
    let Some(source) = &pool.usage else {
        return Outcome::Skipped("the pool names no `usage_provider`".to_owned());
    };
    let Some(variable) = usage::token_env_for(source) else {
        return Outcome::Skipped(
            "the pool names no `usage_token_env`, so there is no credential to ask with".to_owned(),
        );
    };
    if source.provider.needs_account() && source.account.is_none() {
        return Outcome::Skipped(format!(
            "`{}` reports per account and the pool names no `usage_account`",
            source.provider
        ));
    }
    let secret = match credential(variable).map(|value| value.trim().to_owned()) {
        Some(value) if !value.is_empty() => Secret::new(value),
        Some(_) | None => {
            return Outcome::Skipped(format!(
                "the environment variable {} is not set, and it is where the pool says the \
                 credential is",
                variable_as_printed(variable)
            ));
        }
    };
    let request =
        match provider::request(source.provider, source.account.as_deref(), secret, now_unix) {
            Ok(request) => request,
            Err(error) => return Outcome::Skipped(error.to_string()),
        };
    let response = match transport.get(&request) {
        Ok(response) => response,
        Err(error) => return Outcome::Failed(error.to_string()),
    };
    let windows = match provider::parse(source.provider, &response, now_unix) {
        Ok(windows) => windows,
        Err(error) => return Outcome::Failed(error.to_string()),
    };
    let report = UsageReport {
        pool: pool.name.clone(),
        provider: source.provider.to_string(),
        account: usage::account_for(source).map(str::to_owned),
        token_env: Some(variable.to_owned()),
        fetched_at: now_unix,
        windows,
    };
    match publish(store, &report) {
        Ok(_) => Outcome::Stored(report.windows),
        Err(error) => Outcome::Failed(error.to_string()),
    }
}

/// How a diagnostic names the variable a pool's `usage_token_env` gives.
///
/// Whole, when it is no longer than a name the pools reader takes
/// ([`MAX_TOKEN_ENV_CHARS`]). A longer value is far likelier a credential
/// pasted where its variable's name belongs, so it is shown as its first eight
/// characters and its length. The reader refuses such a value before a refresh
/// begins; this is a second guard, for a pool that reaches the tracker some
/// other way, so that a credential never reaches a terminal or a captured log
/// whole.
fn variable_as_printed(variable: &str) -> String {
    let length = variable.chars().count();
    if length <= MAX_TOKEN_ENV_CHARS {
        return format!("`{variable}`");
    }
    let opening: String = variable.chars().take(8).collect();
    format!("`{opening}…` ({length} characters, cut short in case it is a credential)")
}

/// Store `report` as its pool's report under `store`, replacing the one there.
///
/// Atomic, not durable: the bytes are written to a staging file beside the
/// destination, named with a fresh ULID so that two refreshes cannot stage
/// over each other, and renamed into place. If the write or the rename fails,
/// the staging file is removed: it is this call's own, by its name, and a
/// write that fails part way, at a full disk or a file-size limit, leaves part
/// of the report in it. A write that fails before creating it leaves nothing
/// to remove, so the removal's `NotFound` is no failure to clean up. A report
/// the reader would refuse is refused here first and nothing is written.
///
/// # Errors
///
/// [`UpstrokeError::Refused`] for a report outside the schema's bounds, and
/// [`UpstrokeError::Filesystem`] naming the step that failed, or
/// [`UpstrokeError::WithCleanup`] holding it when the staging file could not
/// then be removed either.
pub fn publish(store: &Path, report: &UsageReport) -> Result<PathBuf, UpstrokeError> {
    publish_with(store, report, |staged, encoded| {
        std::fs::write(staged, encoded)
    })
}

/// [`publish`], with the staging file written by `write`, so that a test can
/// make the write leave part of the report and then fail, on every platform
/// and with no file-size limit.
fn publish_with(
    store: &Path,
    report: &UsageReport,
    write: impl FnOnce(&Path, &[u8]) -> std::io::Result<()>,
) -> Result<PathBuf, UpstrokeError> {
    let encoded = report.encode().map_err(|defect| UpstrokeError::Refused {
        message: format!(
            "the report for pool `{}` is not storable: {defect}",
            report.pool
        ),
    })?;
    let destination = usage::report_path(store, &report.pool);
    let failed = |operation: &'static str, path: &Path, source| UpstrokeError::Filesystem {
        operation,
        path: path.to_path_buf(),
        source,
    };
    std::fs::create_dir_all(store)
        .map_err(|source| failed("create the usage report directory", store, source))?;
    let staged = store.join(format!(".staged-{}.tmp", crate::ulid::ulid()));
    let published = write(&staged, encoded.as_bytes())
        .map_err(|source| failed("stage the usage report", &staged, source))
        .and_then(|()| {
            std::fs::rename(&staged, &destination)
                .map_err(|source| failed("publish the usage report", &destination, source))
        });
    if let Err(unpublished) = published {
        let removed = std::fs::remove_file(&staged).or_else(|source| {
            if source.kind() == std::io::ErrorKind::NotFound {
                Ok(())
            } else {
                Err(failed("remove the staged usage report", &staged, source))
            }
        });
        return Err(unpublished.with_cleanup(removed));
    }
    Ok(destination)
}

impl RefreshReport {
    /// The refresh as the lines `upstroke capacity --refresh` prints before
    /// its estimates.
    #[must_use]
    pub fn render(&self) -> String {
        use std::fmt::Write as _;

        if self.pools.is_empty() {
            return "refresh: no pool names a `usage_provider`, so no provider was asked \
                    (DESIGN.md §17)\n"
                .to_owned();
        }
        let mut out = String::from("refresh:\n");
        for refreshed in &self.pools {
            let said = match &refreshed.outcome {
                Outcome::Stored(windows) => {
                    let readings: Vec<String> = windows
                        .iter()
                        .map(|window| {
                            let resets = window.resets_at.map_or_else(String::new, |at| {
                                format!(", resets {}", crate::util::rfc3339_utc(at))
                            });
                            format!("{} used this {}{resets}", window.used, window.label)
                        })
                        .collect();
                    format!("stored — {}", readings.join("; "))
                }
                Outcome::Skipped(why) => format!("skipped — {why}"),
                Outcome::Failed(why) => {
                    format!("failed — {why}; the report stored before, if any, stands")
                }
            };
            let _ = writeln!(out, "  {} [{}]: {said}", refreshed.pool, refreshed.provider);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    use super::provider::{UsageRequest, UsageResponse};
    use super::transport::TransportError;
    use super::*;
    use crate::rundir::scratch_tree::ScratchTree;

    /// 2026-11-23T12:00:00Z.
    const NOON: u64 = 1_795_435_200;

    const POOLS: &str = "\
[pools.anthropic-api]
kind = \"api-key\"
agent = \"claude-code\"
monthly_allowance = 100
usage_provider = \"anthropic-cost\"
usage_token_env = \"ANTHROPIC_ADMIN_KEY\"

[pools.openai-api]
kind = \"api-key\"
agent = \"codex\"
usage_provider = \"openai-cost\"
usage_token_env = \"OPENAI_ADMIN_KEY\"

[pools.copilot]
kind = \"credits\"
agent = \"copilot\"
usage_provider = \"github-ai-credits\"
usage_token_env = \"GH_PLAN_TOKEN\"
usage_account = \"octo-cat\"

[pools.claude-max]
kind = \"subscription-window\"
agent = \"claude-code\"
";

    fn scratch(tag: &str) -> ScratchTree {
        let parent = std::env::temp_dir();
        match crate::rundir::scratch_tree::acquire(&parent, tag) {
            Ok(tree) => tree,
            Err(refusal) => panic!(
                "a scratch tree for `{tag}` under {}: {refusal:?}",
                parent.display()
            ),
        }
    }

    struct Machine {
        tree: ScratchTree,
    }

    impl Machine {
        fn new(tag: &str, pools: &str) -> Self {
            let tree = scratch(tag);
            std::fs::write(tree.path().join("pools.toml"), pools).expect("the pools file");
            Self { tree }
        }

        fn options(&self) -> RefreshOptions {
            RefreshOptions {
                config_path: None,
                pools_path: Some(self.tree.path().join("pools.toml")),
                repo_root: self.tree.path().join("no-repository-here"),
                now_unix: Some(NOON),
            }
        }

        fn store(&self) -> PathBuf {
            usage::store_dir(self.tree.path())
        }

        fn stored(&self, pool: &str) -> Option<UsageReport> {
            usage::read(&self.store(), pool).expect("a stored report reads back")
        }

        /// What the capacity views gather at `now_unix` from the reports
        /// stored so far, with no refresh first, and their estimate of `pool`.
        fn gathered(
            &self,
            pool: &str,
            now_unix: u64,
        ) -> (crate::capacity::Gathered, crate::capacity::PoolEstimate) {
            let options = self.options();
            let mut warnings = Vec::new();
            let config = crate::config::load(
                None,
                &options.repo_root,
                options.pools_path.as_deref(),
                &mut warnings,
            )
            .expect("the pools load");
            let gathered = crate::capacity::machine::gather(
                &crate::capacity::machine::GatherOptions {
                    scope: crate::capacity::Scope::Machine,
                    repo_root: &options.repo_root,
                    machine_dir: config.machine_dir.as_deref(),
                    pools: &config.pools,
                    now_unix,
                },
                &mut warnings,
            );
            let estimate = crate::capacity::estimate_gathered(&config.pools, &gathered)
                .into_iter()
                .find(|estimate| estimate.pool == pool)
                .unwrap_or_else(|| panic!("`{pool}` is estimated"));
            (gathered, estimate)
        }

        /// What `upstroke capacity` reports at `now_unix` from the reports
        /// stored so far, with no refresh first and no agent probed.
        fn capacity_report(&self, now_unix: u64) -> crate::capacity::CapacityReport {
            struct NoAdapters;
            impl crate::agent::AdapterSource for NoAdapters {
                fn get(&self, _id: &str) -> Option<&dyn crate::agent::AgentAdapter> {
                    None
                }
            }
            let options = self.options();
            crate::capacity::report(
                &crate::capacity::CapacityOptions {
                    config_path: None,
                    pools_path: options.pools_path,
                    repo_root: options.repo_root,
                    scope: None,
                    now_unix: Some(now_unix),
                },
                &NoAdapters,
            )
            .expect("the capacity report")
        }
    }

    /// A transport that answers by host and records every request it saw.
    struct Table {
        answers: BTreeMap<&'static str, Result<UsageResponse, TransportError>>,
        asked: RefCell<Vec<UsageRequest>>,
    }

    /// What a host answers: a status and a body, or no response at all.
    type Answer<'a> = Result<(u16, &'a str), TransportError>;

    impl Table {
        fn new(answers: &[(&'static str, Answer<'_>)]) -> Self {
            Self {
                answers: answers
                    .iter()
                    .map(|(host, answer)| {
                        (
                            *host,
                            answer.clone().map(|(status, body)| UsageResponse {
                                status,
                                body: body.to_owned(),
                            }),
                        )
                    })
                    .collect(),
                asked: RefCell::new(Vec::new()),
            }
        }
    }

    impl UsageTransport for Table {
        fn get(&self, request: &UsageRequest) -> Result<UsageResponse, TransportError> {
            self.asked.borrow_mut().push(request.clone());
            self.answers
                .iter()
                .find(|(host, _)| request.url.contains(*host))
                .map_or(Err(TransportError::NoStatus), |(_, answer)| answer.clone())
        }
    }

    fn environment(name: &str) -> Option<String> {
        match name {
            "ANTHROPIC_ADMIN_KEY" => Some("  sk-ant-admin-test\n".to_owned()),
            "OPENAI_ADMIN_KEY" => Some("sk-admin-test".to_owned()),
            "GH_PLAN_TOKEN" => Some("github_pat_test".to_owned()),
            _ => None,
        }
    }

    #[test]
    fn reset_inventory_needs_only_its_dedicated_credential_and_no_pool() {
        let table = Table::new(&[(
            "api.anthropic.com",
            Ok((200, r#"{"cedar_ember":{"eligible":true,"grants":[]}}"#)),
        )]);
        let variables = RefCell::new(Vec::new());
        let report = reset_credits(
            &table,
            &|name| {
                variables.borrow_mut().push(name.to_owned());
                Some("synthetic-reset-secret".to_owned())
            },
            Some(NOON),
        );
        assert_eq!(*variables.borrow(), vec![reset::TOKEN_ENV]);
        assert!(report.render().contains("explicitly empty inventory"));
        assert!(!format!("{report:?}").contains("synthetic-reset-secret"));
        let asked = table.asked.borrow();
        assert_eq!(asked.len(), 1);
        let request = asked.first().expect("one request");
        assert_eq!(
            request.url,
            "https://api.anthropic.com/api/oauth/usage?cedar_ember=1&skip_spend=1"
        );
        assert_eq!(
            request.headers,
            vec![("anthropic-beta", "oauth-2025-04-20".to_owned())]
        );
        assert_eq!(request.credential.header, "Authorization");
        assert_eq!(request.credential.scheme, "Bearer ");
        assert!(!format!("{request:?}").contains("synthetic-reset-secret"));
    }

    #[test]
    fn missing_or_invalid_reset_credentials_send_zero_requests() {
        let table = Table::new(&[]);
        for token in [
            None,
            Some(""),
            Some("secret\nurl = evil"),
            Some(" secret"),
            Some("secret\0"),
        ] {
            let report = reset_credits(&table, &|_| token.map(str::to_owned), Some(NOON));
            let text = report.render();
            assert!(text.contains("unavailable"));
            assert!(text.contains(reset::TOKEN_ENV));
            assert!(text.contains("no request sent"));
            assert!(!text.contains("secret"));
        }
        assert!(table.asked.borrow().is_empty());
    }

    #[test]
    fn reset_failures_never_repeat_process_output_or_credentials() {
        for error in [
            TransportError::NotStarted("synthetic-reset-secret".to_owned()),
            TransportError::Failed {
                code: "synthetic-reset-secret".to_owned(),
                stderr: "synthetic-reset-secret\nforged output".to_owned(),
            },
            TransportError::TimedOut,
            TransportError::TooLarge,
            TransportError::NoStatus,
        ] {
            let table = Table::new(&[("api.anthropic.com", Err(error))]);
            let report = reset_credits(
                &table,
                &|_| Some("synthetic-reset-secret".to_owned()),
                Some(NOON),
            );
            assert!(report.render().contains("unavailable"));
            assert!(!report.render().contains("synthetic-reset-secret"));
            assert!(!format!("{report:?}").contains("synthetic-reset-secret"));
            assert_eq!(table.asked.borrow().len(), 1, "no automatic retry");
        }
    }

    // Runner requires Sync. Each lock protects a single transfer: the scripted
    // answer is consumed once and the owned request snapshot is inspected after
    // the call. Locks never nest and no lock survives across the returned future.
    struct SubscriptionRunner {
        answer: std::sync::Mutex<Option<crate::agent::ProcessOutput>>,
        seen: std::sync::Mutex<Vec<crate::runner::RunnerRequest>>,
    }

    impl crate::runner::Runner for SubscriptionRunner {
        fn run<'a>(
            &'a self,
            request: &'a crate::runner::RunnerRequest,
            _call: crate::runner::RunnerCall<'a>,
        ) -> crate::runner::RunFuture<'a> {
            self.seen.lock().expect("seen lock").push(request.clone());
            let answer = self.answer.lock().expect("answer lock").take();
            let answer = answer.ok_or_else(|| {
                crate::runner::RunnerError::never_started(
                    &request.invocation,
                    UpstrokeError::Refused {
                        message: "synthetic-secret-account".to_owned(),
                    },
                )
            });
            Box::pin(async move { answer })
        }
    }

    fn subscription_output() -> crate::agent::ProcessOutput {
        crate::agent::ProcessOutput {
            code: Some(0),
            stdout: concat!(
                "{\"type\":\"system\",\"account\":\"synthetic-secret-account\"}\n",
                "{\"type\":\"control_response\",\"response\":{\"request_id\":\"usage-only\",",
                "\"subtype\":\"success\",\"response\":{\"rate_limits_available\":true,",
                "\"rate_limits\":{\"five_hour\":{\"utilization\":12},\"cedar_ember\":null}}}}\n"
            )
            .to_owned(),
            stderr: "synthetic-secret-account".to_owned(),
            duration: std::time::Duration::ZERO,
            timed_out: false,
            output_limited: false,
        }
    }

    fn subscription_runner(answer: Option<crate::agent::ProcessOutput>) -> SubscriptionRunner {
        SubscriptionRunner {
            answer: std::sync::Mutex::new(answer),
            seen: std::sync::Mutex::new(Vec::new()),
        }
    }

    #[test]
    fn subscription_cli_is_one_exact_probe_with_its_profile_and_no_bearer() {
        use crate::runner::host::{HostEnvironment, KeyCase};
        use std::ffi::OsString;

        let runner = subscription_runner(Some(subscription_output()));
        let table = Table::new(&[]);
        let report = subscription_usage(
            subscription::Auth::Cli,
            &runner,
            &table,
            &|_| panic!("CLI must never resolve the bearer"),
            Some(NOON),
        );
        assert!(report.render().contains("five-hour: used: 12%"));
        assert!(!report.render().contains("synthetic-secret-account"));
        assert!(table.asked.borrow().is_empty());
        let seen = runner.seen.lock().expect("seen lock");
        assert_eq!(seen.len(), 1);
        let request = seen.first().expect("one probe");
        assert_eq!(request.command.program, "claude");
        assert_eq!(
            request.command.args,
            [
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
                ""
            ]
        );
        assert_eq!(
            std::str::from_utf8(&request.command.stdin).expect("JSON stdin"),
            "{\"type\":\"control_request\",\"request_id\":\"usage-only\",\"request\":{\"subtype\":\"get_usage\",\"skip_behaviors\":true}}\n"
        );
        assert_eq!(request.timeout, transport::PROCESS_TIMEOUT);
        assert_eq!(request.credential_profile, None);
        assert_eq!(
            request.command.env,
            vec![(reset::TOKEN_ENV.to_owned(), String::new())]
        );
        assert!(request.command.settings_file.is_none());
        for profile in [None, Some("/synthetic/profile")] {
            let mut base = vec![
                (
                    OsString::from(reset::TOKEN_ENV),
                    OsString::from("synthetic-bearer"),
                ),
                (
                    OsString::from("CLAUDE_CODE_OAUTH_TOKEN"),
                    OsString::from("synthetic-native"),
                ),
            ];
            if let Some(profile) = profile {
                base.push((OsString::from("CLAUDE_CONFIG_DIR"), OsString::from(profile)));
            }
            let environment = HostEnvironment::with_base(base, KeyCase::Sensitive);
            let composed = environment
                .compose(
                    &request.role,
                    request.agent.as_ref(),
                    None,
                    &request.command.env,
                )
                .expect("compose");
            let value = |key: &str| {
                composed
                    .iter()
                    .find(|(name, _)| name == key)
                    .map(|(_, value)| value.to_str().expect("synthetic ASCII"))
            };
            assert_eq!(value(reset::TOKEN_ENV), Some(""));
            assert_eq!(value("CLAUDE_CONFIG_DIR"), profile);
            assert_eq!(value("CLAUDE_CODE_OAUTH_TOKEN"), Some("synthetic-native"));
        }
    }

    #[test]
    fn subscription_cli_failures_are_advisory_redacted_and_never_fall_back() {
        for failure in 0..8 {
            let mut output = subscription_output();
            let reason = match failure {
                0 => {
                    output.code = Some(1);
                    reset::Unavailable::CliFailed
                }
                1 => {
                    output.code = None;
                    reset::Unavailable::CliFailed
                }
                2 => {
                    output.timed_out = true;
                    reset::Unavailable::CliTimedOut
                }
                3 => {
                    output.output_limited = true;
                    reset::Unavailable::SubscriptionTooLarge
                }
                4 => {
                    output.stdout = "x".repeat(transport::MAX_BODY_BYTES + 1);
                    reset::Unavailable::SubscriptionTooLarge
                }
                5 => {
                    output.stdout = "synthetic-secret-account".to_owned();
                    reset::Unavailable::CliResponse
                }
                6 => {
                    output.stderr = "x".repeat(transport::MAX_BODY_BYTES + 1);
                    reset::Unavailable::SubscriptionTooLarge
                }
                _ => reset::Unavailable::CliNotStarted,
            };
            let runner = subscription_runner((failure != 7).then_some(output));
            let table = Table::new(&[]);
            let report = subscription_usage(
                subscription::Auth::Cli,
                &runner,
                &table,
                &|_| panic!("no fallback"),
                Some(NOON),
            );
            assert_eq!(report, subscription::Report::Unavailable(reason));
            assert!(!report.render().contains("synthetic-secret-account"));
            assert!(table.asked.borrow().is_empty());
            assert_eq!(runner.seen.lock().expect("seen lock").len(), 1);
        }
    }

    #[test]
    fn subscription_bearer_dispatches_the_existing_curl_request_once() {
        let mut output = subscription_output();
        output.stdout =
            "{\"seven_day\":{\"utilization\":31},\"cedar_ember\":{\"grants\":[]}}\n200".to_owned();
        let runner = subscription_runner(Some(output));
        let transport = CurlTransport::new(&runner);
        let report = subscription_usage(
            subscription::Auth::Bearer,
            &runner,
            &transport,
            &|name| {
                assert_eq!(name, reset::TOKEN_ENV);
                Some("synthetic-bearer".to_owned())
            },
            Some(NOON),
        );
        let text = report.render();
        assert!(text.contains("seven-day: used: 31%"));
        assert!(text.contains("explicitly empty inventory"));
        assert!(!text.contains("synthetic"));
        let seen = runner.seen.lock().expect("seen lock");
        assert_eq!(seen.len(), 1);
        let request = seen.first().expect("one curl call");
        assert_eq!(request.command.program, "curl");
        assert_eq!(request.command.args, transport::arguments());
        assert_eq!(
            std::str::from_utf8(&request.command.stdin).expect("config text"),
            "url = \"https://api.anthropic.com/api/oauth/usage?cedar_ember=1&skip_spend=1\"\n\
             header = \"anthropic-beta: oauth-2025-04-20\"\n\
             header = \"Authorization: Bearer synthetic-bearer\"\n"
        );
    }

    #[test]
    fn subscription_bearer_reuses_one_get_and_never_asks_the_cli() {
        for body in [
            r#"{"five_hour":{"utilization":23},"cedar_ember":{"grants":[]}}"#,
            r#"{"cedar_ember":{"grants":[]}}"#,
            r#"{"seven_day":{"utilization":31}}"#,
        ] {
            let runner = subscription_runner(None);
            let table = Table::new(&[("api.anthropic.com", Ok((200, body)))]);
            let report = subscription_usage(
                subscription::Auth::Bearer,
                &runner,
                &table,
                &|name| {
                    assert_eq!(name, reset::TOKEN_ENV);
                    Some("synthetic-bearer".to_owned())
                },
                Some(NOON),
            );
            assert!(matches!(report, subscription::Report::Read { .. }));
            assert!(!report.render().contains("synthetic-bearer"));
            assert!(runner.seen.lock().expect("seen lock").is_empty());
            assert_eq!(table.asked.borrow().len(), 1);
        }
        for (answer, reason) in [
            (
                Err(TransportError::Failed {
                    code: "synthetic-bearer".to_owned(),
                    stderr: "synthetic-bearer".to_owned(),
                }),
                reset::Unavailable::Transport,
            ),
            (
                Ok((401, "synthetic-bearer")),
                reset::Unavailable::CredentialRefused,
            ),
            (Ok((500, "synthetic-bearer")), reset::Unavailable::Status),
            (
                Ok((200, "synthetic-bearer")),
                reset::Unavailable::NotReported,
            ),
        ] {
            let runner = subscription_runner(None);
            let table = Table::new(&[("api.anthropic.com", answer)]);
            let report = subscription_usage(
                subscription::Auth::Bearer,
                &runner,
                &table,
                &|_| Some("synthetic-bearer".to_owned()),
                Some(NOON),
            );
            assert_eq!(report, subscription::Report::Unavailable(reason));
            assert!(!report.render().contains("synthetic-bearer"));
            assert!(runner.seen.lock().expect("seen lock").is_empty());
            assert_eq!(table.asked.borrow().len(), 1);
        }
        for token in [None, Some(""), Some("bad\nsecret")] {
            let runner = subscription_runner(None);
            let table = Table::new(&[]);
            let report = subscription_usage(
                subscription::Auth::Bearer,
                &runner,
                &table,
                &|_| token.map(str::to_owned),
                Some(NOON),
            );
            assert!(report.render().contains("no request sent"));
            assert!(runner.seen.lock().expect("seen lock").is_empty());
            assert!(table.asked.borrow().is_empty());
        }
    }

    const ANTHROPIC_BODY: &str =
        r#"{"data":[{"results":[{"amount":"1250","currency":"USD"}]}],"has_more":false}"#;
    const OPENAI_BODY: &str =
        r#"{"data":[{"results":[{"amount":{"value":3.5,"currency":"usd"}}]}],"has_more":false}"#;
    const GITHUB_BODY: &str = r#"{"usageItems":[{"unitType":"ai-credits","grossQuantity":420}]}"#;

    fn outcome<'a>(report: &'a RefreshReport, pool: &str) -> &'a Outcome {
        &report
            .pools
            .iter()
            .find(|refreshed| refreshed.pool == pool)
            .unwrap_or_else(|| panic!("`{pool}` is in the refresh: {report:?}"))
            .outcome
    }

    #[test]
    fn every_pool_that_names_a_provider_is_asked_and_its_answer_is_stored() {
        let machine = Machine::new("tracker-refresh", POOLS);
        let table = Table::new(&[
            ("api.anthropic.com", Ok((200, ANTHROPIC_BODY))),
            ("api.openai.com", Ok((200, OPENAI_BODY))),
            ("api.github.com", Ok((200, GITHUB_BODY))),
        ]);
        let report = refresh(&machine.options(), &table, &environment).expect("a refresh");

        assert_eq!(
            report
                .pools
                .iter()
                .map(|refreshed| (refreshed.pool.as_str(), refreshed.provider))
                .collect::<Vec<_>>(),
            vec![
                ("anthropic-api", UsageProvider::AnthropicCost),
                ("openai-api", UsageProvider::OpenaiCost),
                ("copilot", UsageProvider::GithubAiCredits),
            ],
            "pools-file order, and the pool that names no provider is not in it"
        );
        for (pool, provider, used) in [
            ("anthropic-api", "anthropic-cost", 12.5),
            ("openai-api", "openai-cost", 3.5),
            ("copilot", "github-ai-credits", 420.0),
        ] {
            let stored = machine.stored(pool).expect("a report was stored");
            assert_eq!(stored.pool, pool);
            assert_eq!(stored.provider, provider);
            assert_eq!(stored.fetched_at, NOON);
            assert_eq!(stored.windows.len(), 1);
            assert_eq!(stored.windows[0].label, "month");
            assert!((stored.windows[0].used - used).abs() < 1e-9, "{stored:?}");
            assert!(
                matches!(outcome(&report, pool), Outcome::Stored(windows) if *windows == stored.windows)
            );
        }
        assert_eq!(machine.stored("claude-max"), None);

        let asked = table.asked.borrow();
        assert_eq!(asked.len(), 3);
        assert_eq!(
            asked[0].credential.secret.expose(),
            "sk-ant-admin-test",
            "a credential read from the environment is trimmed of the line break a shell \
             leaves on it"
        );
        assert!(
            asked[2].url.contains("/users/octo-cat/"),
            "{}",
            asked[2].url
        );
    }

    #[test]
    fn a_stored_report_is_what_the_capacity_views_then_read() {
        let machine = Machine::new("tracker-read-back", POOLS);
        let table = Table::new(&[("api.anthropic.com", Ok((200, ANTHROPIC_BODY)))]);
        refresh(&machine.options(), &table, &environment).expect("a refresh");

        let mut warnings = Vec::new();
        let config = crate::config::load(
            None,
            &machine.options().repo_root,
            machine.options().pools_path.as_deref(),
            &mut warnings,
        )
        .expect("the pools load");
        let gathered = crate::capacity::machine::gather(
            &crate::capacity::machine::GatherOptions {
                scope: crate::capacity::Scope::Machine,
                repo_root: &machine.options().repo_root,
                machine_dir: config.machine_dir.as_deref(),
                pools: &config.pools,
                now_unix: NOON + 3_600,
            },
            &mut warnings,
        );
        let estimates = crate::capacity::estimate_gathered(&config.pools, &gathered);
        let anthropic = estimates
            .iter()
            .find(|estimate| estimate.pool == "anthropic-api")
            .expect("the pool is estimated");
        assert_eq!(anthropic.confidence, crate::capacity::Confidence::Reported);
        assert_eq!(
            anthropic.remaining,
            crate::capacity::Remaining::AtMost(
                (1.0 - 12.5 / 100.0) * (1.0 - crate::capacity::DEFAULT_SAFETY_MARGIN)
                    - crate::capacity::DEFAULT_RESERVE
            ),
            "12.50 of 100 drawn, then the pool's margin and reserve"
        );
        assert_eq!(anthropic.reset_at.as_deref(), Some("2026-12-01T00:00:00Z"));
    }

    /// A pools file whose one pool, `copilot`, reads GitHub's billing for
    /// `account` against an allowance of 100, with no margin and no reserve,
    /// so that the ceiling shown is the provider's reading as it stands.
    fn copilot_for(account: &str) -> String {
        format!(
            "[pools.copilot]\nkind = \"credits\"\nagent = \"copilot\"\nmonthly_allowance = 100\n\
             safety_margin = 0.0\nreserve = 0.0\nusage_provider = \"github-ai-credits\"\n\
             usage_token_env = \"GH_PLAN_TOKEN\"\nusage_account = \"{account}\"\n"
        )
    }

    /// GitHub's answer for an account that has drawn `credits` this month.
    fn github_drew(credits: u32) -> String {
        format!(r#"{{"usageItems":[{{"unitType":"ai-credits","grossQuantity":{credits}}}]}}"#)
    }

    /// Finding PR409-USAGE-CACHE-ACCOUNT-IDENTITY's witness, ported into the
    /// crate. A per-account provider's report is one account's reading: with
    /// `account-a` at 90 of 100 drawn and the pool then switched to
    /// `account-b`, the views showed `account-a`'s "≤10% [provider-reported]"
    /// for `account-b`, with no note, until the window reset.
    #[test]
    fn changing_the_account_must_invalidate_its_previous_report() {
        let machine = Machine::new("tracker-account-switch", &copilot_for("account-a"));
        let ninety = github_drew(90);
        let table = Table::new(&[("api.github.com", Ok((200, ninety.as_str())))]);
        refresh(&machine.options(), &table, &environment).expect("a refresh for account-a");
        assert!(
            table.asked.borrow()[0].url.contains("/users/account-a/"),
            "{}",
            table.asked.borrow()[0].url
        );
        let stored = machine.stored("copilot").expect("a report was stored");
        assert_eq!(
            stored.account.as_deref(),
            Some("account-a"),
            "the report records the account it was fetched for"
        );
        let written = std::fs::read_to_string(usage::report_path(&machine.store(), "copilot"))
            .expect("the stored report reads");
        assert!(
            !written.contains("github_pat_test"),
            "the account's name is stored, never the credential: {written}"
        );

        let later = NOON + 3_600;
        let (gathered, copilot) = machine.gathered("copilot", later);
        assert!(
            gathered.reported.contains_key("copilot"),
            "the pool still names account-a, so its reading binds: {:?}",
            gathered.report_notes
        );
        assert!(
            copilot
                .describe()
                .starts_with("copilot: ≤10% [provider-reported]"),
            "{}",
            copilot.describe()
        );

        std::fs::write(
            machine.tree.path().join("pools.toml"),
            copilot_for("account-b"),
        )
        .expect("the pools file now names account-b");
        let (gathered, copilot) = machine.gathered("copilot", later);
        assert!(
            !gathered.reported.contains_key("copilot"),
            "account-a's reading is not account-b's: {:?}",
            gathered.reported
        );
        assert_eq!(copilot.remaining, crate::capacity::Remaining::Unknown);
        assert!(
            copilot.notes.iter().any(|note| {
                note.contains(
                    "the stored provider report came from account `account-a` and the pool now \
                     names `account-b`, so it bounds nothing",
                ) && note.ends_with("`upstroke capacity --refresh` replaces it")
            }),
            "account-a's reading is not shown for account-b without saying why: {:?}",
            copilot.notes
        );
        assert_eq!(
            table.asked.borrow().len(),
            1,
            "nothing was refreshed in between"
        );

        let thirty = github_drew(30);
        let again = Table::new(&[("api.github.com", Ok((200, thirty.as_str())))]);
        refresh(&machine.options(), &again, &environment).expect("a refresh for account-b");
        assert!(
            again.asked.borrow()[0].url.contains("/users/account-b/"),
            "{}",
            again.asked.borrow()[0].url
        );
        let (gathered, copilot) = machine.gathered("copilot", later);
        assert!(gathered.reported.contains_key("copilot"));
        assert!(
            copilot
                .describe()
                .starts_with("copilot: ≤70% [provider-reported]"),
            "the refresh replaced the report with account-b's own: {}",
            copilot.describe()
        );
        assert!(
            !copilot
                .notes
                .iter()
                .any(|note| note.contains("came from account")),
            "{:?}",
            copilot.notes
        );
    }

    /// A pools file whose one pool, `anthropic-api`, reads Anthropic's cost
    /// report with the admin key in `variable` against an allowance of 100,
    /// with no margin and no reserve.
    fn anthropic_with(variable: &str) -> String {
        format!(
            "[pools.anthropic-api]\nkind = \"api-key\"\nagent = \"claude-code\"\n\
             monthly_allowance = 100\nsafety_margin = 0.0\nreserve = 0.0\n\
             usage_provider = \"anthropic-cost\"\nusage_token_env = \"{variable}\"\n"
        )
    }

    /// Anthropic's answer for an organization that has spent `dollars` this
    /// month, which it reports in cents.
    fn anthropic_spent(dollars: u32) -> String {
        format!(
            r#"{{"data":[{{"results":[{{"amount":"{}","currency":"USD"}}]}}],"has_more":false}}"#,
            dollars * 100
        )
    }

    /// Two organizations' admin keys, each in a variable of its own.
    fn two_organizations(name: &str) -> Option<String> {
        match name {
            "ORG_A_ADMIN_KEY" => Some("sk-ant-admin-org-a-test".to_owned()),
            "ORG_B_ADMIN_KEY" => Some("sk-ant-admin-org-b-test".to_owned()),
            _ => None,
        }
    }

    /// Finding PR409-USAGE-CACHE-CREDENTIAL-IDENTITY's witness, ported into
    /// the crate. A per-key provider records no account, so its report is
    /// one key's organization's reading with nothing to say whose: with the
    /// key in `ORG_A_ADMIN_KEY` at 90 of 100 spent and the pool then switched
    /// to `ORG_B_ADMIN_KEY`, another organization's key, the views showed
    /// organization A's "≤10% [provider-reported]" for it, with no note,
    /// until the window reset.
    #[test]
    fn changing_the_admin_key_variable_invalidates_its_cached_report() {
        let machine = Machine::new("tracker-key-switch", &anthropic_with("ORG_A_ADMIN_KEY"));
        let ninety = anthropic_spent(90);
        let table = Table::new(&[("api.anthropic.com", Ok((200, ninety.as_str())))]);
        refresh(&machine.options(), &table, &two_organizations).expect("a refresh with key A");
        assert_eq!(
            table.asked.borrow()[0].credential.secret.expose(),
            "sk-ant-admin-org-a-test"
        );
        let stored = machine
            .stored("anthropic-api")
            .expect("a report was stored");
        assert_eq!(
            stored.token_env.as_deref(),
            Some("ORG_A_ADMIN_KEY"),
            "the report records the variable its credential was read from"
        );
        assert_eq!(
            stored.account, None,
            "and no account, which it does not read"
        );
        let written =
            std::fs::read_to_string(usage::report_path(&machine.store(), "anthropic-api"))
                .expect("the stored report reads");
        assert!(written.contains("\"ORG_A_ADMIN_KEY\""), "{written}");
        assert!(
            !written.contains("sk-ant-admin"),
            "the variable's name is stored, never the credential: {written}"
        );

        let later = NOON + 3_600;
        let (gathered, anthropic) = machine.gathered("anthropic-api", later);
        assert!(
            gathered.reported.contains_key("anthropic-api"),
            "the pool still names ORG_A_ADMIN_KEY, so its reading binds: {:?}",
            gathered.report_notes
        );
        assert!(
            anthropic
                .describe()
                .starts_with("anthropic-api: ≤10% [provider-reported]"),
            "{}",
            anthropic.describe()
        );

        std::fs::write(
            machine.tree.path().join("pools.toml"),
            anthropic_with("ORG_B_ADMIN_KEY"),
        )
        .expect("the pools file now names ORG_B_ADMIN_KEY");
        let (gathered, anthropic) = machine.gathered("anthropic-api", later);
        assert!(
            !gathered.reported.contains_key("anthropic-api"),
            "organization A's reading is not organization B's: {:?}",
            gathered.reported
        );
        assert_eq!(anthropic.remaining, crate::capacity::Remaining::Unknown);
        assert!(
            anthropic.notes.iter().any(|note| {
                note.contains(
                    "the stored provider report was fetched with the credential in \
                     `ORG_A_ADMIN_KEY` and the pool's `usage_token_env` now names \
                     `ORG_B_ADMIN_KEY`, so it bounds nothing",
                ) && note.ends_with("`upstroke capacity --refresh` replaces it")
            }),
            "organization A's reading is not shown for organization B without saying why: {:?}",
            anthropic.notes
        );
        assert!(
            !anthropic
                .notes
                .iter()
                .any(|note| note.contains("sk-ant-admin")),
            "a note names variables, never a credential: {:?}",
            anthropic.notes
        );
        assert_eq!(
            table.asked.borrow().len(),
            1,
            "nothing was refreshed in between"
        );

        let thirty = anthropic_spent(30);
        let again = Table::new(&[("api.anthropic.com", Ok((200, thirty.as_str())))]);
        refresh(&machine.options(), &again, &two_organizations).expect("a refresh with key B");
        assert_eq!(
            again.asked.borrow()[0].credential.secret.expose(),
            "sk-ant-admin-org-b-test"
        );
        let (gathered, anthropic) = machine.gathered("anthropic-api", later);
        assert!(gathered.reported.contains_key("anthropic-api"));
        assert!(
            anthropic
                .describe()
                .starts_with("anthropic-api: ≤70% [provider-reported]"),
            "the refresh replaced the report with organization B's own: {}",
            anthropic.describe()
        );
        assert!(
            !anthropic
                .notes
                .iter()
                .any(|note| note.contains("fetched with the credential in")),
            "{:?}",
            anthropic.notes
        );
    }

    /// Finding PR409-USAGE-CACHE-MISSING-ACCOUNT's second witness, ported
    /// into the crate: a GitHub report stored before accounts were recorded,
    /// for a pool that names no `usage_account`, as `upstroke capacity` shows
    /// it. It showed "copilot: ≤10% [provider-reported]" with no note, and a
    /// refresh skips the pool, so nothing would ever have replaced it.
    #[test]
    fn missing_identity_is_not_a_ceiling_in_the_capacity_report() {
        let machine = Machine::new(
            "tracker-missing-identity",
            "[pools.copilot]\nkind = \"credits\"\nagent = \"copilot\"\nmonthly_allowance = 100\n\
             safety_margin = 0.0\nreserve = 0.0\nusage_provider = \"github-ai-credits\"\n\
             usage_token_env = \"GH_PLAN_TOKEN\"\n",
        );
        let legacy = serde_json::json!({
            "schema": 1,
            "pool": "copilot",
            "provider": "github-ai-credits",
            "fetched_at": crate::util::rfc3339_utc(NOON),
            "windows": [{
                "label": "month",
                "used_amount": 90.0,
                "resets_at": crate::util::rfc3339_utc(NOON + 200 * 3_600),
            }],
        })
        .to_string();
        let path = usage::report_path(&machine.store(), "copilot");
        std::fs::create_dir_all(machine.store()).expect("the report directory");
        std::fs::write(&path, &legacy).expect("a report stored before accounts were recorded");
        assert_eq!(
            machine
                .stored("copilot")
                .map(|stored| (stored.account, stored.token_env)),
            Some((None, None)),
            "it still decodes, of no known account"
        );

        let report = machine.capacity_report(NOON + 3_600);
        assert!(
            !report.gathered.reported.contains_key("copilot"),
            "a report of no known account is no account's ceiling: {:?}",
            report.gathered.reported
        );
        let copilot = estimate_of(&report, "copilot");
        assert_eq!(copilot.remaining, crate::capacity::Remaining::Unknown);
        let note = "the stored provider report predates account recording and the pool names no \
                    `usage_account`, so which account it measured is not known and it bounds \
                    nothing; `upstroke capacity --refresh` will not run for this pool until it \
                    names one";
        assert!(
            copilot.notes.iter().any(|said| said == note),
            "{:?}",
            copilot.notes
        );
        let rendered = report.render();
        assert!(
            !rendered.contains("[provider-reported]"),
            "the capacity report shows no provider ceiling for the pool: {rendered}"
        );
        assert!(rendered.contains(&format!("  - {note}\n")), "{rendered}");

        let ninety = github_drew(90);
        let table = Table::new(&[("api.github.com", Ok((200, ninety.as_str())))]);
        let refreshed = refresh(&machine.options(), &table, &environment).expect("a refresh");
        match outcome(&refreshed, "copilot") {
            Outcome::Skipped(why) => assert!(why.contains("no `usage_account`"), "{why}"),
            other => panic!("a pool that names no account is skipped, as the note says: {other:?}"),
        }
        assert!(table.asked.borrow().is_empty(), "nothing is asked for it");
        assert_eq!(
            std::fs::read_to_string(&path).expect("the stored report reads"),
            legacy,
            "so the legacy report stands, and stays a note and never a ceiling"
        );
    }

    #[test]
    fn a_pool_with_no_credential_or_no_account_is_skipped_and_nothing_is_asked() {
        let machine = Machine::new(
            "tracker-skips",
            "[pools.no-env]\nkind = \"api-key\"\nagent = \"claude-code\"\n\
             usage_provider = \"anthropic-cost\"\n\n\
             [pools.unset]\nkind = \"api-key\"\nagent = \"codex\"\n\
             usage_provider = \"openai-cost\"\nusage_token_env = \"NOT_IN_THE_TABLE\"\n\n\
             [pools.blank]\nkind = \"api-key\"\nagent = \"codex\"\n\
             usage_provider = \"openai-cost\"\nusage_token_env = \"BLANK\"\n\n\
             [pools.no-account]\nkind = \"credits\"\nagent = \"copilot\"\n\
             usage_provider = \"github-premium-requests\"\nusage_token_env = \"GH_PLAN_TOKEN\"\n",
        );
        let table = Table::new(&[]);
        let blank_too = |name: &str| match name {
            "BLANK" => Some(" \n".to_owned()),
            other => environment(other),
        };
        let report = refresh(&machine.options(), &table, &blank_too).expect("a refresh");
        for (pool, says) in [
            ("no-env", "no `usage_token_env`"),
            ("unset", "`NOT_IN_THE_TABLE` is not set"),
            ("blank", "`BLANK` is not set"),
            ("no-account", "no `usage_account`"),
        ] {
            match outcome(&report, pool) {
                Outcome::Skipped(why) => assert!(why.contains(says), "{pool}: {why}"),
                other => panic!("`{pool}` should be skipped: {other:?}"),
            }
            assert_eq!(machine.stored(pool), None);
        }
        assert!(
            table.asked.borrow().is_empty(),
            "nothing is asked without a credential"
        );
        assert!(
            !machine.store().exists(),
            "and nothing is created to hold no report"
        );
    }

    #[test]
    fn a_failed_refresh_leaves_the_stored_report_as_it_was_and_the_other_pools_refreshed() {
        let machine = Machine::new("tracker-failure", POOLS);
        let first = Table::new(&[
            ("api.anthropic.com", Ok((200, ANTHROPIC_BODY))),
            ("api.openai.com", Ok((200, OPENAI_BODY))),
            ("api.github.com", Ok((200, GITHUB_BODY))),
        ]);
        refresh(&machine.options(), &first, &environment).expect("the first refresh");
        let before = machine.stored("anthropic-api");

        let second = Table::new(&[
            (
                "api.anthropic.com",
                Ok((401, r#"{"error":"invalid x-api-key"}"#)),
            ),
            ("api.openai.com", Err(TransportError::TimedOut)),
            (
                "api.github.com",
                Ok((
                    200,
                    r#"{"usageItems":[{"unitType":"ai-credits","grossQuantity":500}]}"#,
                )),
            ),
        ]);
        let later = RefreshOptions {
            now_unix: Some(NOON + 600),
            ..machine.options()
        };
        let report = refresh(&later, &second, &environment).expect("the second refresh");

        match outcome(&report, "anthropic-api") {
            Outcome::Failed(why) => assert!(why.contains("HTTP 401"), "{why}"),
            other => panic!("a refused credential is a failure: {other:?}"),
        }
        match outcome(&report, "openai-api") {
            Outcome::Failed(why) => assert!(why.contains("did not finish"), "{why}"),
            other => panic!("a timeout is a failure: {other:?}"),
        }
        assert_eq!(machine.stored("anthropic-api"), before);
        assert_eq!(
            machine.stored("openai-api").map(|stored| stored.fetched_at),
            Some(NOON)
        );
        let copilot = machine.stored("copilot").expect("the pool that answered");
        assert_eq!(copilot.fetched_at, NOON + 600);
        assert_eq!(copilot.windows[0].used, 500.0);
    }

    #[test]
    fn publishing_replaces_the_report_whole_and_leaves_no_staging_file() {
        let tree = scratch("tracker-publish");
        let store = usage::store_dir(tree.path());
        let mut report = UsageReport {
            pool: "team/claude".to_owned(),
            provider: "anthropic-cost".to_owned(),
            account: None,
            token_env: Some("ANTHROPIC_ADMIN_KEY".to_owned()),
            fetched_at: NOON,
            windows: vec![UsageWindow {
                label: "month".to_owned(),
                used: 1.0,
                resets_at: None,
            }],
        };
        let first = publish(&store, &report).expect("the first publication");
        report.windows[0].used = 2.0;
        let second = publish(&store, &report).expect("the second replaces it");
        assert_eq!(first, second);
        assert_eq!(
            usage::read(&store, "team/claude").expect("it reads"),
            Some(report)
        );

        let names: Vec<String> = std::fs::read_dir(&store)
            .expect("the store lists")
            .map(|entry| {
                entry
                    .expect("an entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(names.len(), 1, "one report and no staging file: {names:?}");
        assert!(names[0].ends_with(".json"), "{names:?}");
    }

    #[test]
    fn a_report_the_reader_would_refuse_is_never_written() {
        let tree = scratch("tracker-publish-refused");
        let store = usage::store_dir(tree.path());
        let report = UsageReport {
            pool: "p".to_owned(),
            provider: "anthropic-cost".to_owned(),
            account: None,
            token_env: Some("ANTHROPIC_ADMIN_KEY".to_owned()),
            fetched_at: NOON,
            windows: vec![UsageWindow {
                label: "month".to_owned(),
                used: f64::NAN,
                resets_at: None,
            }],
        };
        let error = publish(&store, &report).expect_err("a non-finite amount is not storable");
        assert!(matches!(error, UpstrokeError::Refused { .. }), "{error:?}");
        assert!(!store.exists(), "the refusal is before the first effect");
    }

    #[test]
    fn a_publication_that_cannot_land_is_an_error_and_leaves_nothing_staged() {
        let tree = scratch("tracker-publish-blocked");
        let store = usage::store_dir(tree.path());
        let report = UsageReport {
            pool: "p".to_owned(),
            provider: "anthropic-cost".to_owned(),
            account: None,
            token_env: Some("ANTHROPIC_ADMIN_KEY".to_owned()),
            fetched_at: NOON,
            windows: Vec::new(),
        };
        std::fs::create_dir_all(usage::report_path(&store, "p").join("in-the-way"))
            .expect("a non-empty directory where the report belongs");
        let error = publish(&store, &report).expect_err("a directory cannot be renamed over");
        assert!(
            error.to_string().contains("publish the usage report"),
            "{error}"
        );
        let staged: Vec<String> = std::fs::read_dir(&store)
            .expect("the store lists")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with(".staged-"))
            .collect();
        assert!(staged.is_empty(), "{staged:?}");
    }

    /// The names in the report directory `store`.
    fn names_in(store: &Path) -> Vec<String> {
        std::fs::read_dir(store)
            .expect("the store lists")
            .map(|entry| {
                entry
                    .expect("an entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect()
    }

    /// The witness of a failed staging write, ported into the crate. Under a
    /// 100-byte file-size limit, writing the staged report failed with
    /// `FileTooLarge` and `publish` returned before its one cleanup, which
    /// covered a failed rename alone, so a 100-byte `.staged-*.tmp` stayed in
    /// the store, and another for every refresh that failed the same way. The
    /// writer here leaves half of the report in the staging file and then
    /// fails as that write did, with no limit set.
    #[test]
    fn a_write_that_fails_part_way_leaves_nothing_staged_and_the_published_report_whole() {
        let tree = scratch("tracker-publish-partial");
        let store = usage::store_dir(tree.path());
        let mut report = UsageReport {
            pool: "p".to_owned(),
            provider: "anthropic-cost".to_owned(),
            account: None,
            token_env: Some("ANTHROPIC_ADMIN_KEY".to_owned()),
            fetched_at: NOON,
            windows: vec![UsageWindow {
                label: "month".to_owned(),
                used: 1.0,
                resets_at: None,
            }],
        };
        let destination = publish(&store, &report).expect("the report already published");
        let published = std::fs::read(&destination).expect("the published report reads");

        report.fetched_at = NOON + 600;
        report.windows[0].used = 2.0;
        let mut staged = None;
        let error = publish_with(&store, &report, |path, encoded| {
            staged = Some(path.to_path_buf());
            std::fs::write(path, &encoded[..encoded.len() / 2])?;
            Err(std::io::Error::new(
                std::io::ErrorKind::FileTooLarge,
                "the file-size limit, met half way through the report",
            ))
        })
        .expect_err("a write that fails is the publication's failure");
        let staged = staged.expect("the writer was given the staging file's path");
        match &error {
            UpstrokeError::Filesystem {
                operation,
                path,
                source,
            } => {
                assert_eq!(*operation, "stage the usage report", "{error}");
                assert_eq!(*path, staged, "{error}");
                assert_eq!(source.kind(), std::io::ErrorKind::FileTooLarge, "{error}");
            }
            other => panic!("the write's own error, and no cleanup failure: {other:?}"),
        }
        assert!(
            !staged.exists(),
            "the partial staging file is removed: {}",
            staged.display()
        );
        let names = names_in(&store);
        assert!(
            !names.iter().any(|name| name.starts_with(".staged-")),
            "{names:?}"
        );
        assert_eq!(
            std::fs::read(&destination).expect("the published report still reads"),
            published,
            "the report already published is as it was, byte for byte"
        );
    }

    #[test]
    fn a_write_that_fails_before_creating_the_file_is_its_own_error_and_no_cleanup_failure() {
        let tree = scratch("tracker-publish-uncreated");
        let store = usage::store_dir(tree.path());
        let report = UsageReport {
            pool: "p".to_owned(),
            provider: "anthropic-cost".to_owned(),
            account: None,
            token_env: Some("ANTHROPIC_ADMIN_KEY".to_owned()),
            fetched_at: NOON,
            windows: Vec::new(),
        };
        let mut staged = None;
        let error = publish_with(&store, &report, |path, _| {
            staged = Some(path.to_path_buf());
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "the staging file was refused before it was created",
            ))
        })
        .expect_err("a write that fails is the publication's failure");
        let staged = staged.expect("the writer was given the staging file's path");
        assert!(
            !staged.exists(),
            "the write created nothing: {}",
            staged.display()
        );
        match &error {
            UpstrokeError::Filesystem {
                operation,
                path,
                source,
            } => {
                assert_eq!(*operation, "stage the usage report", "{error}");
                assert_eq!(*path, staged, "{error}");
                assert_eq!(
                    source.kind(),
                    std::io::ErrorKind::PermissionDenied,
                    "{error}"
                );
            }
            other => panic!(
                "a staging file that was never created is no failure to remove it: {other:?}"
            ),
        }
        assert!(names_in(&store).is_empty(), "{:?}", names_in(&store));
    }

    #[test]
    fn the_rendering_says_what_was_stored_skipped_and_failed_and_never_a_credential() {
        let machine = Machine::new("tracker-render", POOLS);
        let table = Table::new(&[
            ("api.anthropic.com", Ok((200, ANTHROPIC_BODY))),
            ("api.openai.com", Ok((500, "upstream"))),
        ]);
        let only_two = |name: &str| match name {
            "GH_PLAN_TOKEN" => None,
            other => environment(other),
        };
        let rendered = refresh(&machine.options(), &table, &only_two)
            .expect("a refresh")
            .render();
        assert!(
            rendered.contains(
                "  anthropic-api [anthropic-cost]: stored — 12.5 used this month, resets \
                 2026-12-01T00:00:00Z"
            ),
            "{rendered}"
        );
        assert!(
            rendered
                .contains("  openai-api [openai-cost]: failed — the provider answered HTTP 500"),
            "{rendered}"
        );
        assert!(
            rendered.contains("  copilot [github-ai-credits]: skipped — the environment variable"),
            "{rendered}"
        );
        for secret in ["sk-ant-admin-test", "sk-admin-test", "github_pat_test"] {
            assert!(!rendered.contains(secret), "{rendered}");
        }

        let none = Machine::new(
            "tracker-render-none",
            "[pools.claude-max]\nkind = \"subscription-window\"\nagent = \"claude-code\"\n",
        );
        let rendered = refresh(&none.options(), &Table::new(&[]), &environment)
            .expect("a refresh with nothing to ask")
            .render();
        assert!(
            rendered.contains("no pool names a `usage_provider`"),
            "{rendered}"
        );
    }

    #[test]
    fn a_refresh_reads_past_settings_it_never_uses() {
        let machine = Machine::new(
            "tracker-refresh-past-engine",
            "[pools.claude-max]\nkind = \"subscription-window\"\nagent = \"claude-code\"\n",
        );
        std::fs::write(
            machine.tree.path().join("upstroke.toml"),
            "[engine]\nmax_paralel = 2\n",
        )
        .expect("the machine-level settings");

        let rendered = match refresh(&machine.options(), &Table::new(&[]), &environment) {
            Ok(report) => report.render(),
            Err(error) => panic!("the refresh refused the machine-level settings: {error}"),
        };
        assert!(
            rendered.contains("no pool names a `usage_provider`"),
            "{rendered}"
        );
    }

    #[test]
    fn a_refresh_refuses_what_its_report_would_before_any_provider_is_asked() {
        struct NoAdapters;
        impl crate::agent::AdapterSource for NoAdapters {
            fn get(&self, _id: &str) -> Option<&dyn crate::agent::AgentAdapter> {
                None
            }
        }

        let machine = Machine::new("tracker-refresh-refuses-as-its-report", POOLS);
        let table = Table::new(&[
            ("api.anthropic.com", Ok((200, ANTHROPIC_BODY))),
            ("api.openai.com", Ok((200, OPENAI_BODY))),
            ("api.github.com", Ok((200, GITHUB_BODY))),
        ]);
        std::fs::write(
            machine.tree.path().join("upstroke.toml"),
            "[capacity]\nscope = \"galaxy\"\n",
        )
        .expect("the machine-level settings");

        let refused = match refresh(&machine.options(), &table, &environment) {
            Ok(report) => panic!("the refresh read a scope its report refuses: {report:?}"),
            Err(refusal) => refusal.to_string(),
        };
        assert!(refused.contains("galaxy"), "{refused}");

        let options = machine.options();
        let reported = match crate::capacity::report(
            &crate::capacity::CapacityOptions {
                config_path: None,
                pools_path: options.pools_path,
                repo_root: options.repo_root,
                scope: None,
                now_unix: options.now_unix,
            },
            &NoAdapters,
        ) {
            Ok(_) => panic!("the report read a scope it refuses"),
            Err(refusal) => refusal.to_string(),
        };
        assert_eq!(
            refused, reported,
            "the refresh refuses with the report's own refusal"
        );

        assert!(
            table.asked.borrow().is_empty(),
            "a provider was asked before the refusal: {:?}",
            table.asked.borrow()
        );
        for pool in ["anthropic-api", "openai-api", "copilot"] {
            assert_eq!(
                machine.stored(pool),
                None,
                "`{pool}` had a report stored before the refusal"
            );
        }
    }

    /// The prefixes a provider's token starts with that the pools reader
    /// refuses in `usage_token_env`, as finding PR409-PASTED-GITHUB-TOKEN-LEAK
    /// lists them.
    const TOKEN_PREFIXES: [&str; 9] = [
        "github_pat_",
        "ghp_",
        "gho_",
        "ghu_",
        "ghs_",
        "ghr_",
        "sk-",
        "sk-ant-",
        "xoxb-",
    ];

    /// A pools file whose one pool gives `value` as its credential's variable
    /// and lacks nothing else a refresh needs.
    fn pools_naming(value: &str) -> String {
        format!(
            "[pools.copilot]\nkind = \"credits\"\nagent = \"copilot\"\n\
             usage_provider = \"github-ai-credits\"\nusage_token_env = \"{value}\"\n\
             usage_account = \"octo-cat\"\n"
        )
    }

    #[test]
    fn a_pasted_github_token_must_not_be_printed_as_a_variable_name() {
        let pasted = format!("github_pat_{}", "A".repeat(82));
        let machine = Machine::new("tracker-pasted-token", &pools_naming(&pasted));
        let table = Table::new(&[]);
        let printed = match refresh(&machine.options(), &table, &environment) {
            Ok(report) => report.render(),
            Err(refusal) => refusal.to_string(),
        };
        assert!(
            !printed.contains(&pasted),
            "what the refresh prints carries the pasted token whole"
        );
        assert!(
            table.asked.borrow().is_empty(),
            "and nothing is asked with it: {printed}"
        );
    }

    #[test]
    fn a_credential_shaped_variable_stops_the_refresh_at_the_pools_file_unrepeated() {
        let mut shaped: Vec<(String, String)> = TOKEN_PREFIXES
            .into_iter()
            .map(|prefix| {
                (
                    format!("a value starting `{prefix}`"),
                    format!("{prefix}EXAMPLE_0nly"),
                )
            })
            .collect();
        shaped.push(("a 65-character value".to_owned(), "K".repeat(65)));
        for (what, value) in &shaped {
            let machine = Machine::new("tracker-credential-shaped", &pools_naming(value));
            let table = Table::new(&[]);
            let refusal = match refresh(&machine.options(), &table, &environment) {
                Ok(_) => panic!("{what} was taken for a variable's name"),
                Err(refusal) => refusal.to_string(),
            };
            assert!(
                !refusal.contains(value.as_str()),
                "the refusal of {what} repeats it"
            );
            assert!(
                refusal.contains(
                    "[pools.copilot] `usage_token_env` must be the NAME of an environment \
                     variable"
                ),
                "the refusal names the pool and says what the key takes: {refusal}"
            );
            assert!(table.asked.borrow().is_empty(), "{refusal}");
        }

        let longest = "K".repeat(64);
        let unset = |_: &str| None::<String>;
        for ordinary in ["GITHUB_TOKEN", "ANTHROPIC_ADMIN_KEY", longest.as_str()] {
            let machine = Machine::new("tracker-ordinary-variable", &pools_naming(ordinary));
            let report = refresh(&machine.options(), &Table::new(&[]), &unset)
                .expect("an ordinary name is a variable's name");
            match outcome(&report, "copilot") {
                Outcome::Skipped(why) => assert!(
                    why.contains(&format!("the environment variable `{ordinary}` is not set")),
                    "the name is printed whole, as before: {why}"
                ),
                other => panic!("`{ordinary}` is unset, so the pool is skipped: {other:?}"),
            }
        }
    }

    #[test]
    fn a_variable_longer_than_the_reader_takes_is_printed_cut_short() {
        let tree = scratch("tracker-cut-short");
        let store = usage::store_dir(tree.path());
        let table = Table::new(&[]);
        let rendered_for = |variable: &str| {
            let pool = Pool {
                usage: Some(crate::capacity::UsageSource {
                    provider: UsageProvider::GithubAiCredits,
                    token_env: Some(variable.to_owned()),
                    account: Some("octo-cat".to_owned()),
                }),
                ..Pool::discovered(
                    "copilot",
                    crate::capacity::PoolKind::Credits,
                    "copilot",
                    Vec::new(),
                )
            };
            RefreshReport {
                pools: vec![PoolRefresh {
                    pool: pool.name.clone(),
                    provider: UsageProvider::GithubAiCredits,
                    outcome: refresh_pool(&pool, &store, &table, &environment, NOON),
                }],
            }
            .render()
        };

        let pasted = format!("github_pat_{}", "A".repeat(82));
        let too_long = "K".repeat(65);
        for (variable, shown) in [
            (
                &pasted,
                "the environment variable `github_p…` (93 characters, cut short in case it is a \
                 credential) is not set",
            ),
            (
                &too_long,
                "the environment variable `KKKKKKKK…` (65 characters, cut short in case it is a \
                 credential) is not set",
            ),
        ] {
            let rendered = rendered_for(variable);
            assert!(
                !rendered.contains(variable.as_str()),
                "a name of {} characters is printed whole",
                variable.len()
            );
            assert!(rendered.contains(shown), "{rendered}");
        }

        let longest = "K".repeat(64);
        let rendered = rendered_for(&longest);
        assert!(
            rendered.contains(&format!("the environment variable `{longest}` is not set")),
            "a name the reader takes is printed whole: {rendered}"
        );
        assert!(
            table.asked.borrow().is_empty(),
            "nothing is asked without a credential"
        );
        assert!(!store.exists(), "and nothing is stored");
    }

    mod fold {
        use std::time::Duration;

        use crate::events::{
            AttemptRecord, AttemptStarted, Event, EventBody, FailureRecord, RunStarted,
        };
        use crate::ladder::{FailureKind, FailureOrigin};

        pub(super) fn record(pool: &str, usd: Option<f64>, limited: bool) -> AttemptRecord {
            AttemptRecord {
                attempt: 1,
                tier: "small".to_owned(),
                model: "m".to_owned(),
                pool: Some(pool.to_owned()),
                resumed: false,
                duration: Duration::ZERO,
                cost_usd: usd,
                reviews: Vec::new(),
                session_id: None,
                usage: None,
                failure: limited.then(|| FailureRecord {
                    kind: FailureKind::RateLimited,
                    origin: FailureOrigin::Worker,
                    reason: "You've hit your 5-hour limit".to_owned(),
                    detail: None,
                }),
            }
        }

        pub(super) fn legacy_started(ts: &str, run_id: &str) -> Event {
            Event {
                ts: ts.to_owned(),
                body: EventBody::RunStarted {
                    data: Box::new(RunStarted {
                        schema: 3,
                        upstroke_version: "0.1.0".to_owned(),
                        run_id: run_id.to_owned(),
                        branch: format!("upstroke/run-{run_id}"),
                        base_sha: "a".repeat(40),
                        plan_path: "plan.md".to_owned(),
                        config_path: None,
                        plan_hash: "h".to_owned(),
                        normalized_plan_digest: Some("d".to_owned()),
                        private_dir: "unused".to_owned(),
                        gates: Vec::new(),
                        gates_from_config: false,
                        interaction_mode: "never".to_owned(),
                        chains: Vec::new(),
                        effort_policy: None,
                        gate_cmds: None,
                        reviews: None,
                    }),
                },
            }
        }

        pub(super) fn legacy_attempt_started(ts: &str, pool: &str) -> Event {
            Event {
                ts: ts.to_owned(),
                body: EventBody::AttemptStarted {
                    task: "t1".to_owned(),
                    attempt: 1,
                    rung: 0,
                    profile: "p".to_owned(),
                    data: AttemptStarted {
                        tier: "small".to_owned(),
                        agent: "claude-code".to_owned(),
                        model: "m".to_owned(),
                        adapter: None,
                        preflight_cli_version: None,
                        effort: None,
                        selection_origin: None,
                        pool: Some(pool.to_owned()),
                        resume_session: None,
                    },
                },
            }
        }

        pub(super) fn legacy_finished(ts: &str, record: AttemptRecord) -> Event {
            Event {
                ts: ts.to_owned(),
                body: EventBody::AttemptFinished {
                    task: "t1".to_owned(),
                    attempt: record.attempt,
                    rung: 0,
                    profile: "p".to_owned(),
                    parking: None,
                    transition: None,
                    prepared_commit: None,
                    data: Box::new(record),
                },
            }
        }

        pub(super) fn lines<T: serde::Serialize>(events: &[T]) -> String {
            events
                .iter()
                .map(|event| {
                    let mut line = serde_json::to_string(event).expect("an event serializes");
                    line.push('\n');
                    line
                })
                .collect()
        }
    }

    /// One machine directory and two repositories that have run on it: `here`
    /// holds a sequential run, found by listing its own run directory, and
    /// `elsewhere` a schema-4 run, found only through the owner record its
    /// private half keeps under the machine directory.
    ///
    /// The schema-4 run is a real one: its log is appended through the event
    /// funnel and accepted by the fold line by line, because the gatherer
    /// reads it with the checked observer, which replays it. Its fixture
    /// stamps every event `2026-11-11T03:00:00Z`. It met a usage limit on
    /// `claude-max` and then landed an attempt that drew $20 on `api`.
    struct TwoRepositories {
        _tree: ScratchTree,
        _elsewhere: crate::capacity::tests::Schema4Run,
        machine: PathBuf,
        here: PathBuf,
        gone: PathBuf,
    }

    const RESET: &str = "2026-11-23T15:00:00Z";
    const ELSEWHERE_RUN: &str = "01TRACKERELSEWHERE00000001";

    impl TwoRepositories {
        fn new(tag: &str) -> Self {
            let tree = scratch(tag);
            let machine = tree.path().join("machine");
            let here = tree.path().join("here");
            let elsewhere = tree.path().join("elsewhere");
            std::fs::create_dir_all(&machine).expect("the machine directory");
            std::fs::write(
                machine.join("pools.toml"),
                "[pools.api]\nkind = \"api-key\"\nagent = \"claude-code\"\nmonthly_allowance = 100\n\n\
                 [pools.claude-max]\nkind = \"subscription-window\"\nagent = \"claude-code\"\n",
            )
            .expect("the pools file");

            let run = crate::capacity::tests::schema_4_run_limited_then_drawing(
                &elsewhere,
                ELSEWHERE_RUN,
                ("claude-max", RESET),
                ("api", 20.0),
            );
            let private = machine.join("runs").join(ELSEWHERE_RUN);
            std::fs::create_dir_all(&private).expect("the run's private half");
            std::fs::write(
                private.join(crate::rundir::OWNER_RECORD),
                serde_json::json!({
                    "run_id": ELSEWHERE_RUN,
                    "repo_key": "0123456789abcdef",
                    "public_dir": crate::rundir::public_dir(&elsewhere, ELSEWHERE_RUN),
                    "incarnation": "i1",
                    "runner": {"a field": "this reader does not need"},
                })
                .to_string(),
            )
            .expect("the owner record");

            let fixture = Self {
                gone: tree.path().join("gone"),
                _tree: tree,
                _elsewhere: run,
                machine,
                here,
            };
            fixture.legacy_run(
                "01HERE",
                &[
                    fold::legacy_started("2026-10-20T09:00:00Z", "01HERE"),
                    fold::legacy_finished(
                        "2026-10-20T10:00:00Z",
                        fold::record("api", Some(50.0), false),
                    ),
                    fold::legacy_finished(
                        "2026-11-10T10:00:00Z",
                        fold::record("api", Some(10.0), false),
                    ),
                ],
            );
            fixture
        }

        /// An owner record under the machine directory, as a run's private
        /// half keeps one.
        fn owner_record(&self, directory: &str, run_id: &str, public_dir: &Path) {
            let private = self.machine.join("runs").join(directory);
            std::fs::create_dir_all(&private).expect("a private half");
            std::fs::write(
                private.join(crate::rundir::OWNER_RECORD),
                serde_json::json!({"run_id": run_id, "public_dir": public_dir}).to_string(),
            )
            .expect("an owner record");
        }

        fn legacy_run(&self, run_id: &str, events: &[crate::events::Event]) {
            let public = crate::rundir::public_dir(&self.here, run_id);
            std::fs::create_dir_all(&public).expect("this repository's run directory");
            std::fs::write(public.join(crate::rundir::EVENT_LOG), fold::lines(events))
                .expect("this repository's log");
        }

        fn report(&self, scope: Option<crate::capacity::Scope>) -> crate::capacity::CapacityReport {
            struct NoAdapters;
            impl crate::agent::AdapterSource for NoAdapters {
                fn get(&self, _id: &str) -> Option<&dyn crate::agent::AgentAdapter> {
                    None
                }
            }
            crate::capacity::report(
                &crate::capacity::CapacityOptions {
                    config_path: None,
                    pools_path: Some(self.machine.join("pools.toml")),
                    repo_root: self.here.clone(),
                    scope,
                    now_unix: Some(NOON),
                },
                &NoAdapters,
            )
            .expect("the capacity report")
        }
    }

    fn estimate_of<'a>(
        report: &'a crate::capacity::CapacityReport,
        pool: &str,
    ) -> &'a crate::capacity::PoolEstimate {
        report
            .estimates
            .iter()
            .find(|estimate| estimate.pool == pool)
            .unwrap_or_else(|| panic!("`{pool}` is estimated"))
    }

    #[test]
    fn capacity_is_folded_from_every_repository_on_the_machine() {
        let fixture = TwoRepositories::new("tracker-machine-fold");
        let report = fixture.report(None);
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        assert_eq!(report.scope, crate::capacity::Scope::Machine);
        assert_eq!(
            (
                report.gathered.runs,
                report.gathered.repositories,
                report.gathered.unread
            ),
            (2, 2, 0)
        );

        let api = estimate_of(&report, "api");
        let drawn = api.self_spend.as_ref().expect("the api pool drew");
        assert_eq!(
            (drawn.usd, drawn.attempts),
            (Some(30.0), 2),
            "this repository's $10 and the other's $20 this month; October's $50 is last \
             month's allowance"
        );
        assert_eq!(api.confidence, crate::capacity::Confidence::SelfMetered);
        assert!(
            api.describe()
                .contains("upstroke on this machine drew, this month, $30.0000"),
            "{}",
            api.describe()
        );

        let subscription = estimate_of(&report, "claude-max");
        assert_eq!(
            subscription.remaining,
            crate::capacity::Remaining::Exhausted,
            "a limit another repository's run met is this pool's state until something serves"
        );
        assert_eq!(subscription.reset_at.as_deref(), Some(RESET));

        let rendered = report.render();
        assert!(
            rendered.contains(
                "scope: machine — signals and self-metered draw are folded from 2 runs across 2 \
                 repositories on this machine"
            ),
            "{rendered}"
        );
    }

    #[test]
    fn the_repo_scope_folds_this_repositorys_runs_alone() {
        let fixture = TwoRepositories::new("tracker-repo-fold");
        let report = fixture.report(Some(crate::capacity::Scope::Repo));
        assert_eq!((report.gathered.runs, report.gathered.repositories), (1, 1));
        let api = estimate_of(&report, "api");
        assert_eq!(
            api.self_spend
                .as_ref()
                .map(|drawn| (drawn.usd, drawn.attempts)),
            Some((Some(10.0), 1))
        );
        assert!(
            api.describe()
                .contains("this repository's runs drew, this month, $10.0000"),
            "{}",
            api.describe()
        );
        assert_eq!(
            estimate_of(&report, "claude-max").remaining,
            crate::capacity::Remaining::Unknown,
            "the other repository's signal is outside this scope"
        );
        assert!(
            report.render().contains("scope: repo — "),
            "{}",
            report.render()
        );

        std::fs::write(
            fixture.machine.join(crate::config::MACHINE_SETTINGS),
            "[capacity]\nscope = \"repo\"\n",
        )
        .expect("machine-level settings");
        let configured = fixture.report(None);
        assert_eq!(configured.scope, crate::capacity::Scope::Repo);
        assert_eq!(configured.gathered.runs, 1);
        let overridden = fixture.report(Some(crate::capacity::Scope::Machine));
        assert_eq!(overridden.gathered.runs, 2, "the flag outranks the setting");
    }

    #[test]
    fn a_later_run_the_pool_serves_retires_another_repositorys_signal() {
        let fixture = TwoRepositories::new("tracker-machine-retire");
        fixture.legacy_run(
            "01HERELATER",
            &[
                fold::legacy_started("2026-11-23T11:00:00Z", "01HERELATER"),
                fold::legacy_attempt_started("2026-11-23T11:00:00Z", "claude-max"),
                fold::legacy_finished(
                    "2026-11-23T11:30:00Z",
                    fold::record("claude-max", None, false),
                ),
            ],
        );
        let report = fixture.report(None);
        assert_eq!(report.gathered.runs, 3);
        assert_eq!(
            estimate_of(&report, "claude-max").remaining,
            crate::capacity::Remaining::Unknown,
            "an attempt the pool served after the refusal proves it is serving again, \
             whichever repository ran it"
        );
    }

    #[test]
    fn a_run_whose_log_cannot_be_read_is_named_and_the_rest_are_still_folded() {
        let fixture = TwoRepositories::new("tracker-machine-unread");
        fixture.owner_record(
            "01GONE",
            "01GONE",
            &crate::rundir::public_dir(&fixture.gone, "01GONE"),
        );
        fixture.owner_record("01MISLABELLED", "01OTHER", &fixture.here);
        fixture.owner_record(
            "01MISPLACED",
            "01MISPLACED",
            &fixture.machine.join("not-a-run-directory"),
        );
        std::fs::create_dir_all(fixture.machine.join("runs").join("01LEGACY"))
            .expect("a private half with no owner record");

        let report = fixture.report(None);
        assert_eq!(
            (
                report.gathered.runs,
                report.gathered.repositories,
                report.gathered.unread
            ),
            (2, 2, 3)
        );
        assert_eq!(report.warnings.len(), 3, "{:?}", report.warnings);
        let warned = |needle: &str| {
            report
                .warnings
                .iter()
                .any(|warning| warning.contains(needle))
        };
        assert!(
            warned("run 01GONE could not be read") && warned("nothing from it is shown"),
            "a run whose repository was deleted is unread, and named: {:?}",
            report.warnings
        );
        assert!(
            warned("names run `01OTHER`"),
            "a record naming another run than its directory is not followed: {:?}",
            report.warnings
        );
        assert!(
            warned("not-a-run-directory") && warned("so it is not followed"),
            "nor is one whose public directory is not a run directory: {:?}",
            report.warnings
        );
        assert_eq!(
            estimate_of(&report, "api")
                .self_spend
                .as_ref()
                .and_then(|drawn| drawn.usd),
            Some(30.0)
        );
        let rendered = report.render();
        assert!(
            rendered.contains(
                "on this machine; the draw counts this calendar month (UTC); 3 more runs could \
                 not be read and nothing from them is shown (the warnings name them)"
            ),
            "the scope line counts what the warnings name: {rendered}"
        );
    }

    /// Production source files, by repository-relative path.
    fn production_sources() -> Vec<(String, String)> {
        fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, String)>) {
            let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
                .unwrap_or_else(|error| panic!("{} lists: {error}", dir.display()))
                .map(|entry| entry.expect("a directory entry").path())
                .collect();
            entries.sort();
            for path in entries {
                if path.is_dir() {
                    walk(&path, root, out);
                } else if path.extension().is_some_and(|extension| extension == "rs") {
                    let relative = path
                        .strip_prefix(root)
                        .expect("a file under the root")
                        .to_string_lossy()
                        .replace('\\', "/");
                    let source = std::fs::read_to_string(&path)
                        .unwrap_or_else(|error| panic!("{} reads: {error}", path.display()));
                    out.push((relative, source));
                }
            }
        }
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut out = Vec::new();
        walk(&root.join("src"), root, &mut out);
        out
    }

    /// The files whose code, with comments and literals blanked, names the
    /// tracker module from outside it.
    fn reaches_the_tracker(sources: &[(String, String)]) -> Vec<&str> {
        sources
            .iter()
            .filter(|(path, _)| path != "src/tracker.rs" && !path.starts_with("src/tracker/"))
            .filter(|(_, source)| {
                let code = crate::effects::blank_comments_and_strings(source);
                code.match_indices("tracker").any(|(at, _)| {
                    let before = code[..at].chars().next_back();
                    let after = code[at + "tracker".len()..].chars().next();
                    let bounded =
                        |c: Option<char>| c.is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
                    bounded(before) && bounded(after)
                })
            })
            .map(|(path, _)| path.as_str())
            .collect()
    }

    #[test]
    fn only_the_cli_reaches_the_tracker() {
        let sources = production_sources();
        assert!(
            sources.len() > 150,
            "only {} source files were read; the census is measuring nothing",
            sources.len()
        );
        for required in [
            "src/engine/coordinator.rs",
            "src/engine/topology/run.rs",
            "src/capacity.rs",
        ] {
            assert!(
                sources.iter().any(|(path, _)| path == required),
                "`{required}` is not in the census's domain"
            );
        }
        assert_eq!(
            reaches_the_tracker(&sources),
            vec!["src/lib.rs", "src/main.rs"],
            "the tracker speaks HTTP and the engine never does (DESIGN.md §4.2): the library \
             root declares the module, the CLI calls its opt-in tracker operations, and nothing \
             else may name it. A run reads the reports the tracker stored; it never asks for one"
        );
    }

    #[test]
    fn the_census_sees_a_call_from_the_engine_and_ignores_prose_about_the_tracker() {
        let planted = vec![
            (
                "src/engine/planted.rs".to_owned(),
                "fn f() { let _ = crate::tracker::refresh; }".to_owned(),
            ),
            (
                "src/engine/aliased.rs".to_owned(),
                "use crate::{tracker as t};".to_owned(),
            ),
            (
                "src/engine/prose.rs".to_owned(),
                "// the tracker stores reports\nconst S: &str = \"tracker\";\nfn trackers() {}\n"
                    .to_owned(),
            ),
            (
                "src/tracker/inside.rs".to_owned(),
                "use crate::tracker::provider;".to_owned(),
            ),
        ];
        assert_eq!(
            reaches_the_tracker(&planted),
            vec!["src/engine/planted.rs", "src/engine/aliased.rs"]
        );
    }
}
