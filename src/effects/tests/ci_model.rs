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

pub(super) const TEST_SCRIPTS: [&str; 3] = [GIT_IDENTITY_SCRIPT, TEST_COMMAND, TEST_SHARD_SCRIPT];

pub(super) const TEST_WINDOWS_SCRIPTS: [&str; 3] = [
    GIT_IDENTITY_SCRIPT,
    WINDOWS_TEST_WITNESS,
    WINDOWS_SHARD_SCRIPT,
];

pub(super) const TEST_SHARD_SCRIPT: &str =
    "python3 .github/scripts/partition.py run macos ${{ matrix.shard }} test-partition";

pub(super) const WINDOWS_SHARD_SCRIPT: &str = "$rustc = ((cargo rustc -q --lib -- --version) | Out-String).Trim(); $path_rustc = ((rustc --version) | Out-String).Trim(); $cargo = ((cargo --version) | Out-String).Trim()\nif ($rustc -notmatch '^rustc 1\\.97\\.1 ' -or $path_rustc -notmatch '^rustc 1\\.97\\.1 ' -or $cargo -notmatch '^cargo 1\\.97\\.1 ') { throw \"the suite would run on '$rustc' (Cargo's compiler), '$path_rustc' (PATH's rustc) and '$cargo', not the image's 1.97.1: a compiler was chosen by something other than this workflow\" }\npython .github/scripts/partition.py run windows ${{ matrix.shard }} test-partition\nif ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }\n";

pub(super) const WINDOWS_COMPILER_CHECK_LINES: usize = 2;

pub(super) const WINDOWS_TEST_FLOOR: u32 = 1700;

pub(super) const WINDOWS_TEST_WITNESS: &str = "$rustc = ((cargo rustc -q --lib -- --version) | Out-String).Trim(); $path_rustc = ((rustc --version) | Out-String).Trim(); $cargo = ((cargo --version) | Out-String).Trim()\nif ($rustc -notmatch '^rustc 1\\.97\\.1 ' -or $path_rustc -notmatch '^rustc 1\\.97\\.1 ' -or $cargo -notmatch '^cargo 1\\.97\\.1 ') { throw \"the suite would run on '$rustc' (Cargo's compiler), '$path_rustc' (PATH's rustc) and '$cargo', not the image's 1.97.1: a compiler was chosen by something other than this workflow\" }\ncargo test --all-targets --all-features | Tee-Object -Variable log\nif ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }\n$passed = [int](($log | Select-String -Pattern '^test result: ok\\. (\\d+) passed' | ForEach-Object { [int]$_.Matches[0].Groups[1].Value } | Measure-Object -Sum).Sum)\nif ($passed -lt 1700) { throw \"the suite reported $passed passing tests, below the floor of 1700: Cargo compiled the harnesses and executed almost none of them\" }\n";

pub(super) const FMT_GATE: &str = "cargo fmt --check";
pub(super) const SHELL_GATES: [&str; 7] = [
    "bash .github/scripts/test-release-record.sh",
    "bash .github/scripts/test-pr-policy.sh",
    "bash .github/scripts/test-pr-ledger-evidence.sh",
    "bash .github/scripts/test-docs-consistency.sh",
    "bash .github/scripts/test-internals-notes.sh",
    "bash .github/scripts/test-pr-ready-audit.sh",
    "bash .github/scripts/test-partition.sh",
];

pub(super) const GATE_SCRIPTS: [&str; 10] = [
    CLIPPY_GATE,
    WINDOWS_BUILD_WITNESS,
    FMT_GATE,
    SHELL_GATES[0],
    SHELL_GATES[1],
    SHELL_GATES[2],
    SHELL_GATES[3],
    SHELL_GATES[4],
    SHELL_GATES[5],
    SHELL_GATES[6],
];

pub(super) const UPLOAD_ACTION: &str =
    "actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a";
pub(super) const DOWNLOAD_ACTION: &str =
    "actions/download-artifact@3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c";

pub(super) const PINNED_ACTIONS: [&str; 5] = [
    "actions/checkout@11d5960a326750d5838078e36cf38b85af677262",
    "dtolnay/rust-toolchain@4360b52568e2003a75bf9bc1d59f33a8e3fc893c",
    "Swatinem/rust-cache@6323deb102c322ba6fcbdcafc7e3dddab59af2b6",
    UPLOAD_ACTION,
    DOWNLOAD_ACTION,
];

pub(super) const ACTION_INPUTS: [(&str, &[&str]); 5] = [
    ("actions/checkout@", &[]),
    ("dtolnay/rust-toolchain@", &["components", "toolchain"]),
    ("Swatinem/rust-cache@", &[]),
    (
        "actions/upload-artifact@",
        &["if-no-files-found", "name", "path"],
    ),
    ("actions/download-artifact@", &["path", "pattern"]),
];

pub(super) const UPLOAD_ACTION_PREFIX: &str = "actions/upload-artifact@";

pub(super) const SHARD_UPLOAD_INPUTS: [(&str, &str); 3] = [
    ("if-no-files-found", "error"),
    (
        "name",
        "test-partition-${{ matrix.shard }}-attempt-${{ github.run_attempt }}",
    ),
    ("path", "test-partition"),
];

pub(super) const TOOLCHAIN_COMPONENTS: [&str; 2] = ["clippy", "rustfmt, clippy"];

pub(super) const TEST_WINDOWS_JOB: &str = "test-windows";
pub(super) const QUEUE_LANE: &str = "github.event_name == 'merge_group'";
pub(super) const HOSTED_LANE: &str = "matrix.shard != 'all'";
pub(super) const WHOLE_SUITE_LANE: &str = "matrix.shard == 'all'";
pub(super) const WHOLE_SUITE: &str = "all";
pub(super) const TEST_WINDOWS_LABELS: [&str; 3] = ["self-hosted", "windows", "winguest"];
pub(super) const TEST_WINDOWS_NAME: &str = "test (${{ matrix.shard == 'all' && 'winguest' || format('windows-latest, {0}', matrix.shard) }})";
pub(super) const TEST_WINDOWS_RUNS_ON: &str = "${{ matrix.shard != 'all' && 'windows-latest' || fromJSON('[\"self-hosted\", \"windows\", \"winguest\"]') }}";
pub(super) const TEST_WINDOWS_TIMEOUT: &str = "${{ matrix.shard != 'all' && 55 || 20 }}";
pub(super) const TEST_NAME: &str = "test (${{ matrix.shard == 'all' && matrix.os || format('{0}, {1}', matrix.os, matrix.shard) }})";
pub(super) const TEST_WHOLE_SUITE_RUNNER: &str = "ubuntu-latest";
pub(super) const TEST_SHARD_RUNNER: &str = "macos-latest";
pub(super) const GOLDEN_IMAGE_TOOLCHAIN: &str = "1.97.1";
pub(super) const TEST_WINDOWS_TOOLCHAIN_COMPONENTS: &str = "clippy";

pub(super) const TEST_WINDOWS_PLATFORM: &str = "windows-latest";

pub(super) const MSRV_JOB: &str = "msrv";
pub(super) const MSRV_COMMAND: &str = "cargo check --locked --all-targets --all-features";

pub(super) const RUSTFLAGS_KEY: &str = "RUSTFLAGS";
pub(super) const RUSTFLAGS_VALUE: &str = "-D warnings";
pub(super) const ENCODED_RUSTFLAGS_KEY: &str = "CARGO_ENCODED_RUSTFLAGS";

pub(super) const AGGREGATE_JOB: &str = "merge-gate";
pub(super) const REQUIRED_CONTEXT: &str = "upstroke-ci";

pub(super) const AGGREGATE_SCRIPT: &str = r#"failed=0
for gate in LINT LINT_WINDOWS LINT_MACOS MSRV TEST TEST_WINDOWS TEST_COVERAGE; do
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

pub(super) const TEST_STEP_ENV: [(&str, &str); 1] = [(
    TEMP_FOLDS_CASE_KEY,
    "${{ matrix.os == 'macos-latest' && '1' || '' }}",
)];

pub(super) const TEST_WINDOWS_STEP_ENV: [(&str, &str); 1] = [(TEMP_FOLDS_CASE_KEY, "1")];

pub(super) const TEST_STEP_FIELDS: [&str; 7] =
    ["env", "if", "name", "run", "shell", "uses", "with"];

pub(super) const LANE_STEP_FIELDS: [&str; 4] = ["if", "name", "uses", "with"];

pub(super) const STRATEGY_FIELDS: [&str; 2] = ["fail-fast", "matrix"];

pub(super) const AGGREGATE_STEP_FIELDS: [&str; 4] = ["env", "name", "run", "shell"];

pub(super) const AGGREGATE_JOB_FIELDS: [&str; 6] =
    ["if", "name", "needs", "runs-on", "steps", "timeout-minutes"];

pub(super) const TEST_JOB_FIELDS: [&str; 5] =
    ["name", "runs-on", "steps", "strategy", "timeout-minutes"];

pub(super) const TEST_WINDOWS_JOB_FIELDS: [&str; 5] =
    ["name", "runs-on", "steps", "strategy", "timeout-minutes"];

pub(super) const COVERAGE_JOB: &str = "test-coverage";
pub(super) const COVERAGE_JOB_NAME: &str = "partition coverage";
pub(super) const COVERAGE_JOB_FIELDS: [&str; 5] =
    ["name", "needs", "runs-on", "steps", "timeout-minutes"];
pub(super) const COVERAGE_NEEDS: [&str; 2] = ["test", TEST_WINDOWS_JOB];
pub(super) const COVERAGE_RUNNER: &str = "ubuntu-latest";
pub(super) const COVERAGE_DOWNLOAD_INPUTS: [(&str, &str); 2] = [
    ("path", "test-partition-records"),
    (
        "pattern",
        "test-partition-*-attempt-${{ github.run_attempt }}",
    ),
];
pub(super) const COVERAGE_COMMAND: &str =
    "python3 .github/scripts/partition.py verify test-partition-records";

pub(super) const PARTITION_TABLE: &str = ".github/scripts/partition.table";
pub(super) const WINDOWS_GROUP: &str = "windows";
pub(super) const MACOS_GROUP: &str = "macos";
pub(super) const PARTITION_EVENTS: [&str; 3] = ["merge_group", "pull_request", "push"];
pub(super) const PARTITION_TABLE_LINES: [&str; 12] = [
    "group windows Windows x86_64-pc-windows-msvc 1.97.1 1700",
    "group macos macOS aarch64-apple-darwin any 1700",
    "lane merge_group windows macos",
    "lane pull_request windows macos",
    "lane push windows macos",
    "shard windows w1 engine::topology::recover::",
    "shard windows w2 engine::topology:: --skip engine::topology::recover::",
    "shard windows w3 engine:: --skip engine::topology::",
    "shard windows w4 --skip engine::",
    "shard macos m1 engine::topology::",
    "shard macos m2 engine:: --skip engine::topology::",
    "shard macos m3 --skip engine::",
];

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
