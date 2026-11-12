//! Extended notes: `docs/internals/effects/tests/ci_model.md`

#![forbid(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

pub(super) const CI_WORKFLOW: &str = ".github/workflows/ci.yml";

pub(super) const CLIPPY_GATE: &str = "cargo clippy --all-targets --all-features -- -D warnings";

pub(super) const TEST_COMMAND: &str = "cargo test --all-targets --all-features";

pub(super) const WINDOWS_BUILD_WITNESS: &str = "cargo build --all-targets --all-features";

pub(super) const GIT_IDENTITY_SCRIPT: &str = "git config --global user.email \"ci@upstroke.local\"\ngit config --global user.name \"upstroke CI\"\n";

pub(super) const TEST_SCRIPTS: [&str; 4] = [
    GIT_IDENTITY_SCRIPT,
    TEST_COMMAND,
    HOST_ACCEPTANCE_CAPTURE,
    CAPABILITY_OBSERVATION,
];

pub(super) const TEST_WINDOWS_SCRIPTS: [&str; 4] = [
    GIT_IDENTITY_SCRIPT,
    WINDOWS_TEST_WITNESS,
    HOST_ACCEPTANCE_WINDOWS_CAPTURE,
    CAPABILITY_WINDOWS_OBSERVATION,
];

pub(super) const TEST_WINDOWS_HOSTED_SCRIPTS: [&str; 4] = [
    GIT_IDENTITY_SCRIPT,
    WINDOWS_HOSTED_TEST_WITNESS,
    HOST_ACCEPTANCE_WINDOWS_CAPTURE,
    CAPABILITY_WINDOWS_OBSERVATION,
];

pub(super) const WINDOWS_TEST_FLOOR: u32 = 1700;

pub(super) const WINDOWS_TEST_WITNESS: &str = "$rustc = ((cargo rustc -q --lib -- --version) | Out-String).Trim(); $path_rustc = ((rustc --version) | Out-String).Trim(); $cargo = ((cargo --version) | Out-String).Trim()\nif ($rustc -notmatch '^rustc 1\\.97\\.1 ' -or $path_rustc -notmatch '^rustc 1\\.97\\.1 ' -or $cargo -notmatch '^cargo 1\\.97\\.1 ') { throw \"the suite would run on '$rustc' (Cargo's compiler), '$path_rustc' (PATH's rustc) and '$cargo', not the image's 1.97.1: a compiler was chosen by something other than this workflow\" }\ncargo test --all-targets --all-features | Tee-Object -Variable log\nif ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }\n$passed = [int](($log | Select-String -Pattern '^test result: ok\\. (\\d+) passed' | ForEach-Object { [int]$_.Matches[0].Groups[1].Value } | Measure-Object -Sum).Sum)\nif ($passed -lt 1700) { throw \"the suite reported $passed passing tests, below the floor of 1700: Cargo compiled the harnesses and executed almost none of them\" }\n";

pub(super) const WINDOWS_HOSTED_TEST_WITNESS: &str = "cargo test --all-targets --all-features | Tee-Object -Variable log\nif ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }\n$passed = [int](($log | Select-String -Pattern '^test result: ok\\. (\\d+) passed' | ForEach-Object { [int]$_.Matches[0].Groups[1].Value } | Measure-Object -Sum).Sum)\nif ($passed -lt 1700) { throw \"the suite reported $passed passing tests, below the floor of 1700: Cargo compiled the harnesses and executed almost none of them\" }\n";

pub(super) const HOST_ACCEPTANCE_CAPTURE: &str = r#"set -o pipefail
log="$RUNNER_TEMP/host-acceptance-record.log"
cargo test --all-features --lib engine::tests::host_acceptance_record -- --ignored --exact --nocapture | tee "$log"
if ! grep -qx 'test engine::tests::host_acceptance_record \.\.\. ok' "$log" || ! grep -q '^test result: ok\. 1 passed; 0 failed; 0 ignored; ' "$log"; then echo "the capture did not run exactly the one host acceptance test to a pass" >&2; exit 1; fi
awk '
  $1 == "UPSTROKE-HOST-ACCEPTANCE-RECORD" { f[++m] = $0 }
  END {
    nb = split(f[1], b, " "); ne = split(f[m], e, " ")
    ok = m >= 3 && nb == 7 && ne == 7 && b[2] == "v1" && b[3] == "BEGIN" && e[2] == "v1" && e[3] == "END"
    ok = ok && b[4] " " b[5] " " b[6] " " b[7] == e[4] " " e[5] " " e[6] " " e[7] && b[5] "" == (m - 2) ""
    for (k = 2; ok && k < m; k++) ok = split(f[k], c, " ") == 5 && c[2] == "v1" && c[3] "" == b[4] "" && c[4] "" == (k - 1) "" && length(c[5]) == (k < m - 1 ? 192 : 2 * (b[6] - (b[5] - 1) * 96))
    exit !ok
  }' "$log" || { echo "the host acceptance record is missing or its framing is incomplete" >&2; exit 1; }
"#;

pub(super) const HOST_ACCEPTANCE_WINDOWS_CAPTURE: &str = r#"cargo test --all-features --lib engine::tests::host_acceptance_record -- --ignored --exact --nocapture | Tee-Object -Variable capture
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
if (-not ($capture -ccontains 'test engine::tests::host_acceptance_record ... ok') -or -not ($capture -cmatch '^test result: ok\. 1 passed; 0 failed; 0 ignored; ')) { throw "the capture did not run exactly the one host acceptance test to a pass" }
$f = @($capture | Where-Object { $_ -cmatch '^UPSTROKE-HOST-ACCEPTANCE-RECORD ' } | ForEach-Object { ,($_ -split ' ') })
$ok = $f.Count -ge 3 -and $f[0].Count -eq 7 -and $f[-1].Count -eq 7 -and $f[0][1] -ceq 'v1' -and $f[0][2] -ceq 'BEGIN' -and $f[-1][1] -ceq 'v1' -and $f[-1][2] -ceq 'END' -and ($f[0][3..6] -join ' ') -ceq ($f[-1][3..6] -join ' ') -and $f[0][4] -ceq "$($f.Count - 2)"
for ($k = 1; $ok -and $k -lt $f.Count - 1; $k++) { $ok = $f[$k].Count -eq 5 -and $f[$k][1] -ceq 'v1' -and $f[$k][2] -ceq $f[0][3] -and $f[$k][3] -ceq "$k" -and $f[$k][4].Length -eq $(if ($k -lt $f.Count - 2) { 192 } else { 2 * ([long]$f[0][5] - ([long]$f[0][4] - 1) * 96) }) }
if (-not $ok) { throw "the host acceptance record is missing or its framing is incomplete" }
"#;

pub(super) const CAPABILITY_OBSERVATION: &str = r#"set -eo pipefail
log="$RUNNER_TEMP/container-capability-record.log"
cargo test --all-features --lib runner::container::tests::container_capability_record -- --ignored --exact --nocapture | tee "$log"
if ! grep -qx 'test runner::container::tests::container_capability_record \.\.\. ok' "$log" || ! grep -q '^test result: ok\. 1 passed; 0 failed; 0 ignored; ' "$log"; then echo "the observation did not run exactly the one container capability test to a pass" >&2; exit 1; fi
awk '
  $1 == "UPSTROKE-CONTAINER-CAPABILITY-RECORD" { f[++m] = $0 }
  END {
    nb = split(f[1], b, " "); ne = split(f[m], e, " ")
    ok = m >= 3 && nb == 7 && ne == 7 && b[2] == "v1" && b[3] == "BEGIN" && e[2] == "v1" && e[3] == "END"
    ok = ok && b[4] " " b[5] " " b[6] " " b[7] == e[4] " " e[5] " " e[6] " " e[7] && b[5] "" == (m - 2) ""
    for (k = 2; ok && k < m; k++) ok = split(f[k], c, " ") == 5 && c[2] == "v1" && c[3] "" == b[4] "" && c[4] "" == (k - 1) "" && length(c[5]) == (k < m - 1 ? 192 : 2 * (b[6] - (b[5] - 1) * 96))
    exit !ok
  }' "$log" || { echo "the container capability record is missing or its framing is incomplete" >&2; exit 1; }
"#;

pub(super) const CAPABILITY_WINDOWS_OBSERVATION: &str = r#"cargo test --all-features --lib runner::container::tests::container_capability_record -- --ignored --exact --nocapture | Tee-Object -Variable capture
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
if (-not ($capture -ccontains 'test runner::container::tests::container_capability_record ... ok') -or -not ($capture -cmatch '^test result: ok\. 1 passed; 0 failed; 0 ignored; ')) { throw "the observation did not run exactly the one container capability test to a pass" }
$f = @($capture | Where-Object { $_ -cmatch '^UPSTROKE-CONTAINER-CAPABILITY-RECORD ' } | ForEach-Object { ,($_ -split ' ') })
$ok = $f.Count -ge 3 -and $f[0].Count -eq 7 -and $f[-1].Count -eq 7 -and $f[0][1] -ceq 'v1' -and $f[0][2] -ceq 'BEGIN' -and $f[-1][1] -ceq 'v1' -and $f[-1][2] -ceq 'END' -and ($f[0][3..6] -join ' ') -ceq ($f[-1][3..6] -join ' ') -and $f[0][4] -ceq "$($f.Count - 2)"
for ($k = 1; $ok -and $k -lt $f.Count - 1; $k++) { $ok = $f[$k].Count -eq 5 -and $f[$k][1] -ceq 'v1' -and $f[$k][2] -ceq $f[0][3] -and $f[$k][3] -ceq "$k" -and $f[$k][4].Length -eq $(if ($k -lt $f.Count - 2) { 192 } else { 2 * ([long]$f[0][5] - ([long]$f[0][4] - 1) * 96) }) }
if (-not $ok) { throw "the container capability record is missing or its framing is incomplete" }
"#;

pub(super) const CAPABILITY_STEP_FIELDS: [&str; 4] = ["env", "name", "run", "timeout-minutes"];

pub(super) const CAPABILITY_STEP_TIMEOUT_MINUTES: i64 = 4;

pub(super) const FMT_GATE: &str = "cargo fmt --check";
pub(super) const SHELL_GATES: [&str; 6] = [
    "bash .github/scripts/test-release-record.sh",
    "bash .github/scripts/test-pr-policy.sh",
    "bash .github/scripts/test-pr-ledger-evidence.sh",
    "bash .github/scripts/test-docs-consistency.sh",
    "bash .github/scripts/test-internals-notes.sh",
    "bash .github/scripts/test-pr-ready-audit.sh",
];

pub(super) const GATE_SCRIPTS: [&str; 9] = [
    CLIPPY_GATE,
    WINDOWS_BUILD_WITNESS,
    FMT_GATE,
    SHELL_GATES[0],
    SHELL_GATES[1],
    SHELL_GATES[2],
    SHELL_GATES[3],
    SHELL_GATES[4],
    SHELL_GATES[5],
];

pub(super) const PINNED_ACTIONS: [&str; 4] = [
    "actions/checkout@11d5960a326750d5838078e36cf38b85af677262",
    "dtolnay/rust-toolchain@4360b52568e2003a75bf9bc1d59f33a8e3fc893c",
    "Swatinem/rust-cache@6323deb102c322ba6fcbdcafc7e3dddab59af2b6",
    "actions/setup-python@ece7cb06caefa5fff74198d8649806c4678c61a1",
];

pub(super) const ACTION_INPUTS: [(&str, &[&str]); 4] = [
    ("actions/checkout@", &[]),
    ("dtolnay/rust-toolchain@", &["components", "toolchain"]),
    ("Swatinem/rust-cache@", &[]),
    ("actions/setup-python@", &["python-version"]),
];

pub(super) const TOOLCHAIN_COMPONENTS: [&str; 2] = ["clippy", "rustfmt, clippy"];

pub(super) const TEST_WINDOWS_JOB: &str = "test-windows";
pub(super) const TEST_WINDOWS_LABELS: [&str; 3] = ["self-hosted", "windows", "winguest"];
pub(super) const GOLDEN_IMAGE_TOOLCHAIN: &str = "1.97.1";

pub(super) const TEST_WINDOWS_PLATFORM: &str = "windows-latest";

pub(super) const TEST_WINDOWS_NAME: &str = "test (winguest)";
pub(super) const TEST_WINDOWS_TIMEOUT_MINUTES: i64 = 20;
pub(super) const TEST_WINDOWS_CONDITION: &str = "needs.pick-windows-home.outputs.home == 'guest'";

pub(super) const TEST_WINDOWS_HOSTED_JOB: &str = "test-windows-hosted";
pub(super) const TEST_WINDOWS_HOSTED_NAME: &str = "test (windows-latest)";
pub(super) const TEST_WINDOWS_HOSTED_TIMEOUT_MINUTES: i64 = 55;
pub(super) const TEST_WINDOWS_HOSTED_CONDITION: &str =
    "needs.pick-windows-home.outputs.home == 'hosted'";

pub(super) const PICK_WINDOWS_HOME_JOB: &str = "pick-windows-home";
pub(super) const PICK_WINDOWS_HOME_NAME: &str = "pick windows home";
pub(super) const PICK_WINDOWS_HOME_RUNNER: &str = "ubuntu-latest";
pub(super) const PICK_WINDOWS_HOME_TIMEOUT_MINUTES: i64 = 5;

pub(super) const PICK_WINDOWS_HOME_JOB_FIELDS: [&str; 6] = [
    "name",
    "outputs",
    "permissions",
    "runs-on",
    "steps",
    "timeout-minutes",
];

pub(super) const PICK_WINDOWS_HOME_PERMISSIONS: [(&str, &str); 2] =
    [("actions", "read"), ("contents", "read")];

pub(super) const PICK_WINDOWS_HOME_STEP_ID: &str = "pick";

pub(super) const PICK_WINDOWS_HOME_OUTPUTS: [(&str, &str); 1] =
    [("home", "${{ steps.pick.outputs.home }}")];

pub(super) const PICK_WINDOWS_HOME_STEP_FIELDS: [&str; 5] = ["env", "id", "name", "run", "shell"];

pub(super) const PICK_WINDOWS_HOME_SHELL: &str = "bash";

pub(super) const PICK_WINDOWS_HOME_STEP_ENV: [(&str, &str); 3] = [
    ("GH_TOKEN", "${{ github.token }}"),
    ("WINGUEST_BUSY", "${{ vars.WINGUEST_BUSY }}"),
    ("WINGUEST_QUEUE_LIMIT", "${{ vars.WINGUEST_QUEUE_LIMIT }}"),
];

pub(super) const WINGUEST_QUEUE_LIMIT_DEFAULT: u32 = 2;

pub(super) const PICK_WINDOWS_HOME_SCRIPT: &str = r#"limit=2
limit_from="the default: WINGUEST_QUEUE_LIMIT is unset or not a positive whole number"
if [[ "${WINGUEST_QUEUE_LIMIT:-}" =~ ^0*[1-9][0-9]{0,5}$ ]]; then
  limit=$((10#$WINGUEST_QUEUE_LIMIT))
  limit_from="WINGUEST_QUEUE_LIMIT"
fi
winguest_jobs() {
  local runs="repos/$GITHUB_REPOSITORY/actions/workflows/ci.yml/runs"
  local status listed seen="" id jobs matched count=0
  local -a run_ids
  for status in queued in_progress; do
    listed="$(gh api --paginate "$runs?status=$status&per_page=100")" || return 1
    listed="$(jq -r '.workflow_runs[].id' <<<"$listed")" || return 1
    seen+="$listed"$'\n'
  done
  mapfile -t run_ids < <(sort -u <<<"$seen")
  for id in "${run_ids[@]}"; do
    if [[ -z "$id" || "$id" == "$GITHUB_RUN_ID" ]]; then continue; fi
    [[ "$id" =~ ^[0-9]+$ ]] || return 1
    jobs="$(gh api --paginate "repos/$GITHUB_REPOSITORY/actions/runs/$id/jobs?filter=latest&per_page=100")" || return 1
    matched="$(jq -s '[.[].jobs[] | select(.name == "test (winguest)" and (.status == "queued" or .status == "in_progress"))] | length' <<<"$jobs")" || return 1
    [[ "$matched" =~ ^[0-9]+$ ]] || return 1
    count=$((count + matched))
  done
  echo "$count"
}
home=guest
echo "limit: $limit ($limit_from)"
if [[ "${WINGUEST_BUSY:-}" == "1" ]]; then
  home=hosted
  echo "WINGUEST_BUSY is 1, so the queue for the guest was not counted."
elif count="$(winguest_jobs)"; then
  echo "WINGUEST_BUSY is not 1. Other runs of this workflow have $count 'test (winguest)' jobs queued or in progress."
  if (( count >= limit )); then home=hosted; fi
else
  echo "WINGUEST_BUSY is not 1, and the GitHub API could not be read, so the queue for the guest is unknown. A failed question does not move a run off the guest."
fi
echo "home: $home"
echo "home=$home" >> "$GITHUB_OUTPUT"
"#;

pub(super) const MSRV_JOB: &str = "msrv";
pub(super) const MSRV_COMMAND: &str = "cargo check --locked --all-targets --all-features";

pub(super) const RUSTFLAGS_KEY: &str = "RUSTFLAGS";
pub(super) const RUSTFLAGS_VALUE: &str = "-D warnings";
pub(super) const ENCODED_RUSTFLAGS_KEY: &str = "CARGO_ENCODED_RUSTFLAGS";

pub(super) const AGGREGATE_JOB: &str = "merge-gate";
pub(super) const REQUIRED_CONTEXT: &str = "upstroke-ci";

pub(super) const AGGREGATE_SCRIPT: &str = r#"failed=0
windows=""
for gate in LINT LINT_WINDOWS LINT_MACOS MSRV PICK_WINDOWS_HOME TEST TEST_WINDOWS TEST_WINDOWS_HOSTED; do
  result_var="${gate}_RESULT"
  result="${!result_var}"
  if [[ "$gate" == "TEST_WINDOWS" || "$gate" == "TEST_WINDOWS_HOSTED" ]]; then
    windows+="$gate=$result "
  elif [[ "$result" != "success" ]]; then
    echo "$gate did not succeed: $result" >&2
    failed=1
  fi
done
case "$windows" in
  "TEST_WINDOWS=success TEST_WINDOWS_HOSTED=skipped " | "TEST_WINDOWS=skipped TEST_WINDOWS_HOSTED=success ") ;;
  *)
    echo "exactly one Windows suite job must succeed and the other be skipped: $windows" >&2
    failed=1
    ;;
esac
exit "$failed"
"#;

pub(super) const GATE_JOB_FIELDS: [&str; 4] = ["name", "runs-on", "steps", "timeout-minutes"];

pub(super) const STEP_FIELDS: [&str; 5] = ["name", "run", "shell", "uses", "with"];

pub(super) const TEMP_FOLDS_CASE_KEY: &str = "UPSTROKE_TEST_TEMP_FOLDS_CASE";

pub(super) const TEST_STEP_ENV: [(&str, &str); 1] = [(
    TEMP_FOLDS_CASE_KEY,
    "${{ matrix.os == 'macos-latest' && '1' || '' }}",
)];

pub(super) const TEST_WINDOWS_STEP_ENV: [(&str, &str); 1] = [(TEMP_FOLDS_CASE_KEY, "1")];

pub(super) const TEST_STEP_FIELDS: [&str; 6] = ["env", "name", "run", "shell", "uses", "with"];

pub(super) const AGGREGATE_STEP_FIELDS: [&str; 4] = ["env", "name", "run", "shell"];

pub(super) const AGGREGATE_JOB_FIELDS: [&str; 6] =
    ["if", "name", "needs", "runs-on", "steps", "timeout-minutes"];

pub(super) const TEST_JOB_FIELDS: [&str; 5] =
    ["name", "runs-on", "steps", "strategy", "timeout-minutes"];

pub(super) const TEST_WINDOWS_JOB_FIELDS: [&str; 6] =
    ["if", "name", "needs", "runs-on", "steps", "timeout-minutes"];

pub(super) const MSRV_JOB_FIELDS: [&str; 5] =
    ["name", "runs-on", "steps", "strategy", "timeout-minutes"];

pub(super) const OPTIONAL_DEFAULTS_FIELD: [&str; 1] = ["defaults"];

pub(super) const WORKFLOW_FIELDS: [&str; 6] =
    ["concurrency", "env", "jobs", "name", "on", "permissions"];

pub(super) const DEFAULTS_FIELDS: [&str; 1] = ["run"];
pub(super) const DEFAULTS_RUN_FIELDS: [&str; 1] = ["shell"];

pub(super) const WORKFLOW_ENV: [(&str, &str); 2] = [
    ("CARGO_TERM_COLOR", "always"),
    (RUSTFLAGS_KEY, RUSTFLAGS_VALUE),
];

pub(super) const WORKFLOW_PERMISSIONS: [(&str, &str); 1] = [("contents", "read")];

pub(super) const OVERRIDING_REPO_FILES: [&str; 4] = [
    "rust-toolchain.toml",
    "rust-toolchain",
    ".cargo/config.toml",
    ".cargo/config",
];

pub(super) const TOOLCHAIN_ACTION: &str = "dtolnay/rust-toolchain@";
pub(super) const STABLE_TOOLCHAIN: &str = "stable";

pub(super) const KNOWN_SHELLS: [&str; 6] = ["bash", "cmd", "powershell", "pwsh", "python", "sh"];

pub(super) const AGGREGATE_SHELL: &str = "bash";

pub(super) struct CiTarget {
    pub(super) runner: &'static str,
    pub(super) triple: &'static str,
    pub(super) default_shell: &'static str,
    pub(super) keys: &'static [(&'static str, &'static str)],
    pub(super) flags: &'static [&'static str],
    pub(super) per_invocation_flags: &'static [&'static [&'static str]],
}

pub(super) const CI_TARGETS: [CiTarget; 3] = [
    CiTarget {
        runner: "ubuntu-latest",
        default_shell: "bash",
        triple: "x86_64-unknown-linux-gnu",
        keys: &[
            ("target_os", "linux"),
            ("target_family", "unix"),
            ("target_env", "gnu"),
            ("target_arch", "x86_64"),
            ("target_vendor", "unknown"),
            ("target_pointer_width", "64"),
            ("target_endian", "little"),
        ],
        flags: &["unix"],
        per_invocation_flags: &[&[], &["test"]],
    },
    CiTarget {
        runner: "macos-latest",
        default_shell: "bash",
        triple: "aarch64-apple-darwin",
        keys: &[
            ("target_os", "macos"),
            ("target_family", "unix"),
            ("target_env", ""),
            ("target_arch", "aarch64"),
            ("target_vendor", "apple"),
            ("target_pointer_width", "64"),
            ("target_endian", "little"),
        ],
        flags: &["unix"],
        per_invocation_flags: &[&[], &["test"]],
    },
    CiTarget {
        runner: "windows-latest",
        default_shell: "pwsh",
        triple: "x86_64-pc-windows-msvc",
        keys: &[
            ("target_os", "windows"),
            ("target_family", "windows"),
            ("target_env", "msvc"),
            ("target_arch", "x86_64"),
            ("target_vendor", "pc"),
            ("target_pointer_width", "64"),
            ("target_endian", "little"),
        ],
        flags: &["windows"],
        per_invocation_flags: &[&[], &["test"]],
    },
];
