//! Extended notes: `docs/internals/effects/tests/ci_model.md`

#![forbid(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

pub(super) const CI_WORKFLOW: &str = ".github/workflows/ci.yml";

pub(super) const CLIPPY_GATE: &str = "cargo clippy --all-targets --all-features -- -D warnings";

pub(super) const CLIPPY_GATE_MACOS_CROSS: &str =
    "cargo clippy --target aarch64-apple-darwin --all-targets --all-features -- -D warnings";

pub(super) const TEST_COMMAND: &str = "cargo test --all-targets --all-features";

pub(super) const TEST_SHARD_COMMAND: &str =
    "cargo test --all-targets --all-features -- ${{ matrix.tests }}";

pub(super) const WINDOWS_BUILD_WITNESS: &str = "cargo build --all-targets --all-features";

pub(super) const GIT_IDENTITY_SCRIPT: &str = "git config --global user.email \"ci@upstroke.local\"\ngit config --global user.name \"upstroke CI\"\n";

pub(super) const TEST_SCRIPTS: [&str; 4] = [
    GIT_IDENTITY_SCRIPT,
    TEST_SHARD_COMMAND,
    HOST_ACCEPTANCE_CAPTURE,
    CAPABILITY_OBSERVATION,
];

pub(super) const TEST_WINDOWS_HOSTED_SCRIPTS: [&str; 4] = [
    GIT_IDENTITY_SCRIPT,
    WINDOWS_SHARD_WITNESS,
    HOST_ACCEPTANCE_WINDOWS_CAPTURE,
    CAPABILITY_WINDOWS_OBSERVATION,
];

pub(super) const WINDOWS_TEST_FLOOR: u32 = 1700;

pub(super) const WINDOWS_SHARD_WITNESS: &str = r#"$rustc = ((cargo rustc -q --lib -- --version) | Out-String).Trim(); $path_rustc = ((rustc --version) | Out-String).Trim(); $cargo = ((cargo --version) | Out-String).Trim()
if ($rustc -notmatch '^rustc 1\.97\.1 ' -or $path_rustc -notmatch '^rustc 1\.97\.1 ' -or $cargo -notmatch '^cargo 1\.97\.1 ') { throw "the suite would run on '$rustc' (Cargo's compiler), '$path_rustc' (PATH's rustc) and '$cargo', not the image's 1.97.1: a compiler was chosen by something other than this workflow" }
$out = Join-Path $env:RUNNER_TEMP 'suite-stdout.log'; $err = Join-Path $env:RUNNER_TEMP 'suite-stderr.log'; $built = Join-Path $env:RUNNER_TEMP 'suite-build.json'
$build = Start-Process -FilePath cargo -ArgumentList 'test --all-targets --all-features --no-run --message-format=json-render-diagnostics' -NoNewWindow -PassThru -RedirectStandardOutput $built -RedirectStandardError $err; $null = $build.Handle; $build.WaitForExit(); $code = $build.ExitCode
if ($code -ne 0) { try { Get-Content $err } catch { "reading the build's errors failed: $_" }; exit $code }
$harnesses = @(Get-Content $built | ForEach-Object { $_ | ConvertFrom-Json } | Where-Object { $_.reason -ceq 'compiler-artifact' -and $_.profile.test -and $_.executable } | ForEach-Object { $_.executable }); if ($harnesses.Count -eq 0) { throw "Cargo built no test harness, so this shard would run nothing" }
$env:RUSTC_BOOTSTRAP = '1'; foreach ($h in $harnesses) { $suite = Start-Process -FilePath $h -ArgumentList '${{ matrix.tests }} -Z unstable-options --report-time' -NoNewWindow -PassThru -RedirectStandardOutput "$out.part" -RedirectStandardError "$err.part"; $null = $suite.Handle; $suite.WaitForExit(); $code = $suite.ExitCode; try { Get-Content "$out.part" | Add-Content $out } catch { "copying the output of $h failed: $_"; if ($code -eq 0) { throw } }; try { Get-Content "$err.part" | Add-Content $err } catch { "copying the errors of $h failed: $_"; if ($code -eq 0) { throw } }; if ($code -ne 0) { break } }
try { Get-Content $err; $log = Get-Content $out; $log; $left = @(Get-CimInstance Win32_Process -Filter "Name LIKE 'upstroke%'"); foreach ($p in $left) { "left running after the suite: pid $($p.ProcessId): $($p.CommandLine)"; Stop-Process -Id $p.ProcessId -Force -ErrorAction SilentlyContinue } }
catch { if ($code -ne 0) { "reading the suite's output or listing the processes it left failed: $_" } else { throw } }
if ($code -ne 0) { exit $code }
if ($left.Count -gt 0) { throw "$($left.Count) test process(es) outlived the suite, listed above: a test leaves a process running, which would hold this step open" }
$passed = [int](($log | Select-String -Pattern '^test result: ok\. (\d+) passed' | ForEach-Object { [int]$_.Matches[0].Groups[1].Value } | Measure-Object -Sum).Sum)
if ($passed -lt ${{ matrix.floor }}) { throw "shard ${{ matrix.shard }} reported $passed passing tests, below its floor of ${{ matrix.floor }}: Cargo compiled the harnesses and executed almost none of this shard's tests" }
"shard ${{ matrix.shard }} reported $passed passing tests; its floor is ${{ matrix.floor }}"
"#;

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

pub(super) const GATE_SCRIPTS: [&str; 10] = [
    CLIPPY_GATE,
    CLIPPY_GATE_MACOS_CROSS,
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
    (
        "dtolnay/rust-toolchain@",
        &["components", "targets", "toolchain"],
    ),
    ("Swatinem/rust-cache@", &[]),
    ("actions/setup-python@", &["python-version"]),
];

pub(super) const TOOLCHAIN_COMPONENTS: [&str; 2] = ["clippy", "rustfmt, clippy"];

pub(super) const TOOLCHAIN_TARGETS: [&str; 2] = ["aarch64-apple-darwin", MSRV_TOOLCHAIN_TARGETS];

pub(super) const GOLDEN_IMAGE_TOOLCHAIN: &str = "1.97.1";

pub(super) const TEST_WINDOWS_PLATFORM: &str = "windows-latest";

pub(super) const TEST_WINDOWS_HOSTED_JOB: &str = "test-windows-hosted";
pub(super) const TEST_WINDOWS_HOSTED_NAME: &str = "test (windows-latest, ${{ matrix.shard }})";
pub(super) const TEST_WINDOWS_HOSTED_TIMEOUT_MINUTES: i64 = 40;
pub(super) const CACHE_ACTION: &str = "Swatinem/rust-cache@";

pub(super) const WINDOWS_SHARD_FILTERS: [&[&str]; 13] = [
    &[
        "engine::topology::coordinator::tests::a_h",
        "engine::topology::coordinator::tests::t",
        "engine::topology::recover::tests::k",
        "workspace_manager::tests::c",
        "engine::tests::a_t",
    ],
    &[
        "effects::tests::every_pr6_refusal_st16_variant_and_invariant_clause_names_a_test_or_an_owner",
        "engine::topology::recover::tests::a_k",
        "engine::topology::recover::tests::an",
        "engine::tests::a_p",
        "runner::contract::",
        "engine::topology::coordinator::tests::o",
    ],
    &[
        "engine::topology::coordinator::tests::interleaving::s",
        "engine::topology::dispatch::",
        "workspace_manager::tests::a",
        "runner::container::",
        "engine::tests::a_d",
        "engine::tests::a_s",
    ],
    &[
        "engine::topology::coordinator::tests::a_p",
        "engine::topology::recover::tests::a_f",
        "engine::topology::candidate::",
        "workspace_manager::tests::r",
    ],
    &[
        "engine::topology::coordinator::tests::independent_tasks_dispatch_together_and_keep_per_key_projections_under_every_seed",
        "effects::tests::every_test_the_container_fault_row_names_is_a_test_in_this_tree",
        "engine::topology::coordinator::tests::a_d",
        "engine::topology::coordinator::tests::a_g",
        "engine::tests::t",
        "engine::topology::coordinator::tests::a_v",
    ],
    &[
        "engine::topology::coordinator::tests::interleaving::r",
        "engine::topology::coordinator::tests::a_c",
        "engine::tests::a_b",
        "engine::tests::e",
        "engine::tests::r",
        "engine::tests::s",
        "engine::topology::recover::tests::s",
    ],
    &[
        "engine::topology::coordinator::tests::a_f",
        "engine::topology::coordinator::tests::d",
        "workspace_manager::tests::e",
        "workspace_manager::tests::t",
        "engine::tests::a_l",
    ],
    &[
        "engine::topology::coordinator::tests::a_k",
        "engine::topology::coordinator::tests::a_s",
        "engine::topology::coordinator::tests::r",
        "engine::topology::coordinator::tests::s",
        "engine::topology::integrate::",
        "engine::tests::l",
        "engine::topology::coordinator::tests::c",
    ],
    &[
        "engine::topology::coordinator::tests::an",
        "engine::topology::recover::tests::a_r",
        "engine::topology::coverage::",
        "engine::tests::a_r",
    ],
    &[
        "engine::topology::coordinator::tests::a_r",
        "engine::topology::finalize::",
        "engine::topology::ledger::",
        "engine::tests::a_k",
    ],
    &[
        "engine::topology::coordinator::tests::interleaving::i",
        "engine::topology::coordinator::tests::ap",
        "engine::topology::run::",
        "topology::census::",
        "engine::tests::an",
        "engine::topology::recover::tests::t",
    ],
    &[
        "engine::topology::coordinator::tests::interleaving::a",
        "engine::topology::recover::tests::r",
        "workspace_manager::tests::n",
        "workspace::",
    ],
    &[
        "engine::topology::attempt::",
        "workspace_manager::tests::s",
        "engine::tests::a_c",
        "engine::tests::a_f",
        "engine::tests::f",
    ],
];

pub(super) const WINDOWS_SHARD_FLOORS: [u32; 14] =
    [22, 44, 224, 43, 27, 33, 36, 35, 54, 26, 83, 45, 43, 985];

pub(super) const MACOS_SHARD_FILTERS: [&[&str]; 4] = [
    &[
        "engine::topology::coordinator::tests::a_p",
        "workspace_manager::tests::t",
        "engine::tests::a_k",
        "engine::tests::a_r",
        "engine::tests::a_s",
        "runner::host::",
        "workspace::tests::",
    ],
    &[
        "engine::topology::coordinator::tests::interleaving::",
        "engine::topology::coordinator::tests::a_s",
        "workspace_manager::tests::e",
        "workspace_manager::tests::n",
        "topology::census::",
        "effects::tests::",
    ],
    &[
        "engine::topology::coordinator::tests::a_r",
        "engine::topology::recover::tests::a_",
        "engine::topology::recover::tests::k",
        "workspace_manager::tests::s",
        "engine::tests::a_b",
        "engine::tests::t",
        "engine::topology::recover::",
        "workspace_manager::tests::a",
        "events::log::",
    ],
    &[
        "engine::topology::coordinator::tests::a_g",
        "engine::topology::coordinator::tests::an",
        "runner::container::",
        "runner::contract::",
        "engine::tests::an",
        "engine::topology::attempt::",
    ],
];

pub(super) const LINUX_SHARD_FILTERS: [&[&str]; 1] = [&["engine::"]];

pub(super) const TEST_JOB_NAME: &str = "test (${{ matrix.leg }})";
pub(super) const TEST_TIMEOUT_MINUTES: i64 = 30;

pub(super) struct TestJob {
    pub(super) id: &'static str,
    pub(super) runner: &'static str,
    pub(super) filters: &'static [&'static [&'static str]],
    pub(super) step_env: [(&'static str, &'static str); 1],
}

pub(super) const TEST_JOBS: [TestJob; 2] = [
    TestJob {
        id: "test-linux",
        runner: "ubuntu-latest",
        filters: &LINUX_SHARD_FILTERS,
        step_env: [(TEMP_FOLDS_CASE_KEY, "")],
    },
    TestJob {
        id: "test-macos",
        runner: "macos-latest",
        filters: &MACOS_SHARD_FILTERS,
        step_env: [(TEMP_FOLDS_CASE_KEY, "1")],
    },
];

pub(super) const RETIRED_TEST_JOB: &str = "test";

pub(super) const MSRV_JOB: &str = "msrv";
pub(super) const MSRV_COMMAND: &str =
    "cargo check --locked --target ${{ matrix.target }} --all-targets --all-features";
pub(super) const MSRV_MATRIX_FIELDS: [&str; 1] = ["include"];
pub(super) const MSRV_ENTRY_KEYS: [&str; 2] = ["os", "target"];
pub(super) const MSRV_RUNS_ON: &str = "${{ matrix.os }}";
pub(super) const MSRV_TOOLCHAIN_TARGETS: &str = "${{ matrix.target }}";

pub(super) const RUSTFLAGS_KEY: &str = "RUSTFLAGS";
pub(super) const RUSTFLAGS_VALUE: &str = "-D warnings";
pub(super) const ENCODED_RUSTFLAGS_KEY: &str = "CARGO_ENCODED_RUSTFLAGS";

pub(super) const AGGREGATE_JOB: &str = "merge-gate";
pub(super) const REQUIRED_CONTEXT: &str = "upstroke-ci";

pub(super) const AGGREGATE_SCRIPT: &str = r#"failed=0
for gate in LINT LINT_WINDOWS LINT_MACOS MSRV TEST_LINUX TEST_MACOS TEST_WINDOWS_HOSTED; do
  result_var="${gate}_RESULT"
  result="${!result_var}"
  if [[ "$result" != "success" ]]; then
    echo "$gate did not succeed: $result" >&2
    failed=1
  fi
done
exit "$failed"
"#;

pub(super) const GATE_JOB_FIELDS: [&str; 4] = ["name", "runs-on", "steps", "timeout-minutes"];

pub(super) const STEP_FIELDS: [&str; 5] = ["name", "run", "shell", "uses", "with"];

pub(super) const TEMP_FOLDS_CASE_KEY: &str = "UPSTROKE_TEST_TEMP_FOLDS_CASE";

pub(super) const TEST_WINDOWS_STEP_ENV: [(&str, &str); 1] = [(TEMP_FOLDS_CASE_KEY, "1")];

pub(super) const TEST_STEP_FIELDS: [&str; 6] = ["env", "name", "run", "shell", "uses", "with"];

pub(super) const AGGREGATE_STEP_FIELDS: [&str; 4] = ["env", "name", "run", "shell"];

pub(super) const AGGREGATE_JOB_FIELDS: [&str; 6] =
    ["if", "name", "needs", "runs-on", "steps", "timeout-minutes"];

pub(super) const TEST_JOB_FIELDS: [&str; 5] =
    ["name", "runs-on", "steps", "strategy", "timeout-minutes"];

pub(super) const TEST_WINDOWS_HOSTED_JOB_FIELDS: [&str; 5] =
    ["name", "runs-on", "steps", "strategy", "timeout-minutes"];

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
    pub(super) check_runner: &'static str,
    pub(super) msrv_runner: &'static str,
    pub(super) clippy_gate: &'static str,
    pub(super) triple: &'static str,
    pub(super) default_shell: &'static str,
    pub(super) keys: &'static [(&'static str, &'static str)],
    pub(super) flags: &'static [&'static str],
    pub(super) per_invocation_flags: &'static [&'static [&'static str]],
}

pub(super) const CI_TARGETS: [CiTarget; 3] = [
    CiTarget {
        runner: "ubuntu-latest",
        check_runner: "ubuntu-latest",
        msrv_runner: "ubuntu-latest",
        clippy_gate: CLIPPY_GATE,
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
        check_runner: "ubuntu-latest",
        msrv_runner: "macos-latest",
        clippy_gate: CLIPPY_GATE_MACOS_CROSS,
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
        check_runner: "windows-latest",
        msrv_runner: "windows-latest",
        clippy_gate: CLIPPY_GATE,
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
