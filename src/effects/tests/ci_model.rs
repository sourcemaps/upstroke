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

pub(super) const CATEGORY_CHECK: &str = "bash .github/scripts/test-categories.sh";

pub(super) const CATEGORY_CHECK_WINDOWS: &str = "python .github/scripts/test-categories.py check";

pub(super) const CATEGORY_CHECK_MACOS: &str = r#"if [[ '${{ matrix.leg }}' == 'macos-latest, unit ('* ]]; then
  bash .github/scripts/test-categories.sh
fi
"#;

pub(super) const TEST_SCRIPTS: [&str; 6] = [
    GIT_IDENTITY_SCRIPT,
    CATEGORY_CHECK,
    CATEGORY_CHECK_MACOS,
    TEST_SHARD_COMMAND,
    HOST_ACCEPTANCE_CAPTURE,
    CAPABILITY_OBSERVATION,
];

pub(super) const TEST_WINDOWS_HOSTED_SCRIPTS: [&str; 5] = [
    GIT_IDENTITY_SCRIPT,
    CATEGORY_CHECK_WINDOWS,
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
pub(super) const SHELL_GATES: [&str; 7] = [
    "bash .github/scripts/test-release-record.sh",
    "bash .github/scripts/test-pr-policy.sh",
    "bash .github/scripts/test-pr-ledger-evidence.sh",
    "bash .github/scripts/test-docs-consistency.sh",
    "bash .github/scripts/test-internals-notes.sh",
    "bash .github/scripts/test-pr-ready-audit.sh",
    CATEGORY_CHECK,
];

pub(super) const GATE_SCRIPTS: [&str; 11] = [
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
    SHELL_GATES[6],
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

#[derive(Debug, serde::Serialize)]
pub(super) struct TestCategory {
    pub(super) category: &'static str,
    pub(super) subcategory: &'static str,
    pub(super) filters: &'static [&'static str],
    pub(super) macos: u8,
    pub(super) windows: &'static [(u8, u32, &'static [&'static str])],
}

pub(super) const TEST_CATEGORIES: &[TestCategory] = &[
    TestCategory {
        category: "kill-path",
        subcategory: "samplers",
        filters: &["_child_kills_"],
        macos: 1,
        windows: &[(1, 2, &[])],
    },
    TestCategory {
        category: "kill-path",
        subcategory: "matrices",
        filters: &[
            "kill_at_every_cell",
            "kill_after_report_before_each_cleanup_step",
            "_stopped_at_any_phase_",
        ],
        macos: 1,
        windows: &[
            (
                1,
                1,
                &[
                    "engine::topology::coordinator::tests::interleaving::a_concurrent_halted_run_finalizes_through_the_frozen_finalization_and_converges_after_a_kill_at_every_cell_",
                ],
            ),
            (
                2,
                3,
                &[
                    "engine::topology::recover::tests::a_kill_at_every_cell_of_a_",
                    "engine::topology::recover::tests::kill_after_report_before_each_cleanup_step_before_the_effect",
                    "engine::topology::finalize::tests::a_cross_kind_scrub_stopped_at_any_phase_converges_on_the_next",
                ],
            ),
            (
                3,
                3,
                &[
                    "engine::topology::coordinator::tests::interleaving::a_concurrent_complete_run_finalizes_through_the_frozen_finalization_and_converges_after_a_kill_at_every_cell_",
                    "engine::topology::recover::tests::kill_after_report_before_each_cleanup_step_after_the_effect",
                    "workspace_manager::tests::incarnations::r_o2_a_cross_kind_scrub_stopped_at_any_phase_",
                    "workspace_manager::tests::reclaim::a_torn_reclaim_stopped_at_any_phase_",
                ],
            ),
        ],
    },
    TestCategory {
        category: "coordinator",
        subcategory: "interleaving",
        filters: &["engine::topology::coordinator::tests::interleaving::"],
        macos: 2,
        windows: &[(2, 7, &[])],
    },
    TestCategory {
        category: "coordinator",
        subcategory: "pipelines",
        filters: &["engine::topology::coordinator::tests::pipelines::"],
        macos: 1,
        windows: &[(5, 13, &[])],
    },
    TestCategory {
        category: "coordinator",
        subcategory: "registry_waits",
        filters: &["engine::topology::coordinator::tests::registry_waits::"],
        macos: 2,
        windows: &[(1, 21, &[])],
    },
    TestCategory {
        category: "coordinator",
        subcategory: "runners",
        filters: &["engine::topology::coordinator::tests::runners::"],
        macos: 2,
        windows: &[(4, 4, &[])],
    },
    TestCategory {
        category: "coordinator",
        subcategory: "scheduling",
        filters: &["engine::topology::coordinator::tests::scheduling::"],
        macos: 2,
        windows: &[(6, 11, &[])],
    },
    TestCategory {
        category: "coordinator",
        subcategory: "shutdown",
        filters: &["engine::topology::coordinator::tests::shutdown::"],
        macos: 2,
        windows: &[(5, 19, &[])],
    },
    TestCategory {
        category: "coordinator",
        subcategory: "snapshot_registration",
        filters: &["engine::topology::coordinator::tests::snapshot_registration::"],
        macos: 1,
        windows: &[
            (
                1,
                3,
                &[
                    "engine::topology::coordinator::tests::snapshot_registration::a_passing_",
                    "engine::topology::coordinator::tests::snapshot_registration::an_",
                    "engine::topology::coordinator::tests::snapshot_registration::a_clean_",
                ],
            ),
            (
                2,
                5,
                &[
                    "engine::topology::coordinator::tests::snapshot_registration::a_retained_",
                    "engine::topology::coordinator::tests::snapshot_registration::a_resume_",
                    "engine::topology::coordinator::tests::snapshot_registration::the_",
                    "engine::topology::coordinator::tests::snapshot_registration::a_live_",
                    "engine::topology::coordinator::tests::snapshot_registration::a_promotions_",
                ],
            ),
            (
                3,
                7,
                &[
                    "engine::topology::coordinator::tests::snapshot_registration::a_gate_",
                    "engine::topology::coordinator::tests::snapshot_registration::a_verification_",
                    "engine::topology::coordinator::tests::snapshot_registration::a_genuinely_",
                    "engine::topology::coordinator::tests::snapshot_registration::a_container_",
                    "engine::topology::coordinator::tests::snapshot_registration::a_classification_",
                    "engine::topology::coordinator::tests::snapshot_registration::a_refusal_",
                    "engine::topology::coordinator::tests::snapshot_registration::a_halted_",
                ],
            ),
            (
                4,
                3,
                &[
                    "engine::topology::coordinator::tests::snapshot_registration::a_cancelled_",
                    "engine::topology::coordinator::tests::snapshot_registration::a_stale_",
                    "engine::topology::coordinator::tests::snapshot_registration::a_staging_",
                    "engine::topology::coordinator::tests::snapshot_registration::a_kept_",
                    "engine::topology::coordinator::tests::snapshot_registration::a_complete_",
                ],
            ),
            (
                6,
                4,
                &["engine::topology::coordinator::tests::snapshot_registration::a_review_"],
            ),
        ],
    },
    TestCategory {
        category: "coordinator",
        subcategory: "verification_charges",
        filters: &["engine::topology::coordinator::tests::verification_charges::"],
        macos: 1,
        windows: &[(4, 11, &[])],
    },
    TestCategory {
        category: "coordinator",
        subcategory: "core",
        filters: &["engine::topology::coordinator::"],
        macos: 1,
        windows: &[(2, 3, &[])],
    },
    TestCategory {
        category: "engine",
        subcategory: "recover",
        filters: &["engine::topology::recover::"],
        macos: 1,
        windows: &[
            (
                1,
                15,
                &[
                    "engine::topology::recover::tests::a_resume_",
                    "engine::topology::recover::tests::a_one_",
                    "engine::topology::recover::tests::picking_",
                    "engine::topology::recover::tests::a_test_",
                    "engine::topology::recover::tests::a_checkouts_",
                    "engine::topology::recover::tests::every_",
                    "engine::topology::recover::tests::barrier_",
                    "engine::topology::recover::tests::unsynced_",
                    "engine::topology::recover::tests::append_",
                    "engine::topology::recover::tests::malformed_",
                    "engine::topology::recover::tests::a_prepared_",
                    "engine::topology::recover::tests::a_refused_",
                    "engine::topology::recover::tests::closure_",
                ],
            ),
            (
                2,
                22,
                &[
                    "engine::topology::recover::tests::a_fault_",
                    "engine::topology::recover::tests::resume_",
                    "engine::topology::recover::tests::a_torn_",
                    "engine::topology::recover::tests::a_lost_",
                    "engine::topology::recover::tests::two_",
                    "engine::topology::recover::tests::a_verification_",
                    "engine::topology::recover::tests::a_git_",
                    "engine::topology::recover::tests::finalized_",
                    "engine::topology::recover::tests::a_budget_",
                    "engine::topology::recover::tests::a_delivered_",
                    "engine::topology::recover::tests::over_",
                    "engine::topology::recover::tests::a_log_",
                    "engine::topology::recover::tests::a_missing_",
                ],
            ),
            (
                3,
                22,
                &[
                    "engine::topology::recover::tests::an_",
                    "engine::topology::recover::tests::run_",
                    "engine::topology::recover::tests::t_",
                    "engine::topology::recover::tests::repeated_",
                    "engine::topology::recover::tests::declining_",
                    "engine::topology::recover::tests::a_retained_",
                    "engine::topology::recover::tests::a_clean_",
                    "engine::topology::recover::tests::late_",
                    "engine::topology::recover::tests::a_dispatch_",
                    "engine::topology::recover::tests::projections_",
                    "engine::topology::recover::tests::a_recorded_",
                    "engine::topology::recover::tests::a_crash_",
                    "engine::topology::recover::tests::a_plan_",
                    "engine::topology::recover::tests::a_call_",
                    "engine::topology::recover::tests::candidate_",
                    "engine::topology::recover::tests::container_",
                    "engine::topology::recover::tests::finalization_",
                    "engine::topology::recover::tests::process_",
                    "engine::topology::recover::tests::recovery_",
                    "engine::topology::recover::tests::staging_",
                ],
            ),
            (
                4,
                6,
                &[
                    "engine::topology::recover::tests::a_repair_",
                    "engine::topology::recover::tests::max_",
                    "engine::topology::recover::tests::synthetic_",
                    "engine::topology::recover::tests::a_reviewer_",
                    "engine::topology::recover::tests::a_fresh_",
                    "engine::topology::recover::tests::a_completed_",
                    "engine::topology::recover::tests::a_live_",
                    "engine::topology::recover::tests::answer_",
                    "engine::topology::recover::tests::a_runs_",
                    "engine::topology::recover::tests::a_run_",
                    "engine::topology::recover::tests::replayed_",
                    "engine::topology::recover::tests::a_rebound_",
                ],
            ),
            (
                5,
                18,
                &[
                    "engine::topology::recover::tests::the_",
                    "engine::topology::recover::tests::a_report_",
                    "engine::topology::recover::tests::a_failed_",
                    "engine::topology::recover::tests::private_",
                    "engine::topology::recover::tests::each_",
                    "engine::topology::recover::tests::a_paid_",
                    "engine::topology::recover::tests::a_binding_",
                    "engine::topology::recover::tests::a_removal_",
                    "engine::topology::recover::tests::halted_",
                    "engine::topology::recover::tests::a_retried_",
                    "engine::topology::recover::tests::steps_",
                    "engine::topology::recover::tests::ledgers_",
                    "engine::topology::recover::tests::a_fixture_",
                ],
            ),
            (
                6,
                21,
                &[
                    "engine::topology::recover::tests::a_kill_",
                    "engine::topology::recover::tests::fresh_",
                    "engine::topology::recover::tests::a_gate_",
                    "engine::topology::recover::tests::kill_",
                    "engine::topology::recover::tests::a_ref_",
                    "engine::topology::recover::tests::a_rejected_",
                    "engine::topology::recover::tests::a_repairs_",
                    "engine::topology::recover::tests::a_dependent_",
                    "engine::topology::recover::tests::a_runner_",
                    "engine::topology::recover::tests::a_closed_",
                    "engine::topology::recover::tests::retry_",
                    "engine::topology::recover::tests::forged_",
                    "engine::topology::recover::tests::a_build_",
                ],
            ),
        ],
    },
    TestCategory {
        category: "engine",
        subcategory: "capacity_budget",
        filters: &["engine::tests::capacity_budget::"],
        macos: 1,
        windows: &[(4, 17, &[])],
    },
    TestCategory {
        category: "engine",
        subcategory: "kept_output",
        filters: &["engine::tests::kept_output::"],
        macos: 2,
        windows: &[(3, 19, &[])],
    },
    TestCategory {
        category: "engine",
        subcategory: "legacy_resume",
        filters: &["engine::tests::legacy_resume::"],
        macos: 1,
        windows: &[(2, 27, &[])],
    },
    TestCategory {
        category: "engine",
        subcategory: "questions",
        filters: &["engine::tests::questions::"],
        macos: 2,
        windows: &[(6, 17, &[])],
    },
    TestCategory {
        category: "engine",
        subcategory: "snapshot_registration",
        filters: &["engine::tests::snapshot_registration::"],
        macos: 1,
        windows: &[(4, 12, &[])],
    },
    TestCategory {
        category: "engine",
        subcategory: "topology_admission",
        filters: &["engine::tests::topology_admission::"],
        macos: 1,
        windows: &[(4, 8, &[])],
    },
    TestCategory {
        category: "engine",
        subcategory: "topology_execution",
        filters: &["engine::tests::topology_execution::"],
        macos: 2,
        windows: &[(6, 8, &[])],
    },
    TestCategory {
        category: "engine",
        subcategory: "verification",
        filters: &["engine::tests::verification::"],
        macos: 1,
        windows: &[(5, 27, &[])],
    },
    TestCategory {
        category: "engine",
        subcategory: "end-to-end",
        filters: &["engine::tests::"],
        macos: 2,
        windows: &[(5, 2, &[])],
    },
    TestCategory {
        category: "engine",
        subcategory: "attempt",
        filters: &["engine::topology::attempt::"],
        macos: 2,
        windows: &[(5, 25, &[])],
    },
    TestCategory {
        category: "engine",
        subcategory: "topology",
        filters: &[
            "engine::topology::run::",
            "engine::topology::integrate::",
            "engine::topology::dispatch::",
            "engine::topology::coverage::",
            "engine::topology::ledger::",
            "engine::topology::candidate::",
            "engine::topology::finalize::",
            "engine::topology::create::",
            "engine::topology::startup::",
            "engine::topology::repair::",
            "engine::topology::permits::",
            "engine::topology::emit::",
            "engine::topology::settle::",
            "engine::topology::entry::",
            "engine::topology::identity::",
            "engine::topology::preflight::",
            "engine::topology::prelock::",
            "engine::topology::profiled::",
            "engine::topology::seams::",
            "engine::topology::select::",
        ],
        macos: 2,
        windows: &[(1, 213, &[])],
    },
    TestCategory {
        category: "engine",
        subcategory: "report",
        filters: &["engine::report::"],
        macos: 2,
        windows: &[(2, 4, &[])],
    },
    TestCategory {
        category: "workspace",
        subcategory: "containment",
        filters: &["workspace_manager::tests::containment::"],
        macos: 1,
        windows: &[(1, 23, &[])],
    },
    TestCategory {
        category: "workspace",
        subcategory: "guarded_reads",
        filters: &["workspace_manager::tests::guarded_reads::"],
        macos: 1,
        windows: &[(2, 16, &[])],
    },
    TestCategory {
        category: "workspace",
        subcategory: "incarnations",
        filters: &["workspace_manager::tests::incarnations::"],
        macos: 1,
        windows: &[(1, 11, &[])],
    },
    TestCategory {
        category: "workspace",
        subcategory: "reclaim",
        filters: &["workspace_manager::tests::reclaim::"],
        macos: 1,
        windows: &[(2, 12, &[])],
    },
    TestCategory {
        category: "workspace",
        subcategory: "refs",
        filters: &["workspace_manager::tests::refs::"],
        macos: 1,
        windows: &[(2, 10, &[])],
    },
    TestCategory {
        category: "workspace",
        subcategory: "registry",
        filters: &["workspace_manager::tests::registry::"],
        macos: 1,
        windows: &[(2, 14, &[])],
    },
    TestCategory {
        category: "workspace",
        subcategory: "residue",
        filters: &["workspace_manager::tests::residue::"],
        macos: 1,
        windows: &[(1, 16, &[])],
    },
    TestCategory {
        category: "workspace",
        subcategory: "snapshots",
        filters: &["workspace_manager::tests::snapshots::"],
        macos: 1,
        windows: &[(2, 13, &[])],
    },
    TestCategory {
        category: "workspace",
        subcategory: "manager",
        filters: &["workspace_manager::"],
        macos: 1,
        windows: &[(2, 29, &[])],
    },
    TestCategory {
        category: "workspace",
        subcategory: "checkout",
        filters: &["workspace::tests::"],
        macos: 1,
        windows: &[(1, 22, &[])],
    },
    TestCategory {
        category: "runner",
        subcategory: "container",
        filters: &["runner::container::"],
        macos: 1,
        windows: &[(1, 116, &[])],
    },
    TestCategory {
        category: "runner",
        subcategory: "contract",
        filters: &["runner::contract::"],
        macos: 1,
        windows: &[(1, 13, &[])],
    },
    TestCategory {
        category: "runner",
        subcategory: "host",
        filters: &["runner::host::"],
        macos: 1,
        windows: &[(1, 41, &[])],
    },
    TestCategory {
        category: "runner",
        subcategory: "policy",
        filters: &["runner::invocation::", "runner::policy::"],
        macos: 1,
        windows: &[(1, 13, &[])],
    },
    TestCategory {
        category: "unit",
        subcategory: "model",
        filters: &[
            "topology::fold::",
            "topology::effects::",
            "topology::events::",
            "topology::registry::",
            "topology::schema::",
            "topology::paths::",
        ],
        macos: 1,
        windows: &[(1, 193, &[])],
    },
    TestCategory {
        category: "self-checks",
        subcategory: "effects",
        filters: &["effects::tests::"],
        macos: 1,
        windows: &[(1, 46, &[])],
    },
    TestCategory {
        category: "self-checks",
        subcategory: "census",
        filters: &["topology::census::"],
        macos: 1,
        windows: &[(1, 18, &[])],
    },
    TestCategory {
        category: "unit",
        subcategory: "agent-proc",
        filters: &["agent::proc::"],
        macos: 1,
        windows: &[(1, 34, &[])],
    },
    TestCategory {
        category: "unit",
        subcategory: "agent-adapters",
        filters: &[
            "agent::adapter::",
            "agent::bin::",
            "agent::claude::",
            "agent::codex::",
            "agent::copilot::",
        ],
        macos: 1,
        windows: &[(1, 63, &[])],
    },
    TestCategory {
        category: "unit",
        subcategory: "config",
        filters: &["config::", "capacity::", "tracker::", "connect::"],
        macos: 1,
        windows: &[(1, 152, &[])],
    },
    TestCategory {
        category: "unit",
        subcategory: "plan",
        filters: &[
            "plan::",
            "validate::",
            "ir::tests::",
            "ladder::",
            "catalog::",
            "route::",
        ],
        macos: 1,
        windows: &[(1, 104, &[])],
    },
    TestCategory {
        category: "unit",
        subcategory: "run-state",
        filters: &[
            "rundir::",
            "events::",
            "review::",
            "gates::",
            "interaction::",
            "answer::",
            "status::",
            "export::",
        ],
        macos: 1,
        windows: &[(1, 120, &[])],
    },
    TestCategory {
        category: "unit",
        subcategory: "util",
        filters: &["util::", "ulid::", "error::"],
        macos: 1,
        windows: &[(1, 12, &[])],
    },
    TestCategory {
        category: "unit",
        subcategory: "cli",
        filters: &["cli_tests::"],
        macos: 1,
        windows: &[(1, 6, &[])],
    },
];

#[derive(Debug, serde::Serialize)]
pub(super) struct CategoryLeg {
    pub(super) name: String,
    pub(super) tests: String,
    pub(super) floor: u32,
}

pub(super) fn category_legs(platform: &str) -> Vec<CategoryLeg> {
    let mut rows = Vec::new();
    for row in TEST_CATEGORIES {
        if platform == "windows" {
            for &(leg, floor, filters) in row.windows {
                rows.push((
                    row,
                    leg,
                    floor,
                    if filters.is_empty() {
                        row.filters
                    } else {
                        filters
                    },
                ));
            }
        } else {
            assert!(
                platform == "macos" || platform == "ubuntu",
                "unknown platform {platform}"
            );
            rows.push((
                row,
                if platform == "macos" { row.macos } else { 1 },
                0,
                row.filters,
            ));
        }
    }
    let categories: std::collections::BTreeSet<&str> =
        TEST_CATEGORIES.iter().map(|row| row.category).collect();
    let mut legs = Vec::new();
    for category in categories {
        let count = rows
            .iter()
            .filter(|(row, _, _, _)| row.category == category)
            .map(|(_, leg, _, _)| *leg)
            .max()
            .expect("category has rows");
        for leg in 1..=count {
            let owns = |row: &TestCategory, number: u8| row.category == category && number == leg;
            let selected: Vec<_> = rows
                .iter()
                .filter(|(row, number, _, _)| owns(row, *number))
                .collect();
            let filters: Vec<&str> = selected
                .iter()
                .flat_map(|(_, _, _, filters)| filters.iter().copied())
                .collect();
            let mut skips: Vec<&str> = TEST_CATEGORIES
                .iter()
                .take_while(|row| row.category != category)
                .flat_map(|row| row.filters.iter().copied())
                .collect();
            for (index, (row, number, _, excluded)) in rows.iter().enumerate() {
                if row.category != category || owns(row, *number) {
                    continue;
                }
                for filter in *excluded {
                    if rows
                        .iter()
                        .skip(index + 1)
                        .any(|(later, number, _, included)| {
                            owns(later, *number) && included.iter().any(|own| filter.contains(own))
                        })
                    {
                        skips.push(filter);
                    }
                }
            }
            let mut words: Vec<String> = Vec::new();
            for filter in &filters {
                if !filters
                    .iter()
                    .any(|other| other != filter && filter.contains(other))
                    && !words.iter().any(|word| word == filter)
                {
                    words.push((*filter).to_owned());
                }
            }
            assert!(
                !words.is_empty(),
                "{category}/{leg} has no positive selector"
            );
            let mut skipped = std::collections::BTreeSet::new();
            for skip in skips {
                if skipped.insert(skip) {
                    words.extend(["--skip".to_owned(), skip.to_owned()]);
                }
            }
            let subs: std::collections::BTreeSet<&str> = selected
                .iter()
                .map(|(row, _, _, _)| row.subcategory)
                .collect();
            let suffix = if count == 1 {
                String::new()
            } else {
                format!(" {leg} of {count}")
            };
            legs.push(CategoryLeg {
                name: format!(
                    "{category}{suffix} ({})",
                    subs.into_iter().collect::<Vec<_>>().join(", ")
                ),
                tests: words.join(" "),
                floor: selected.iter().map(|(_, _, floor, _)| *floor).sum(),
            });
        }
    }
    legs
}

pub(super) const TEST_JOB_NAME: &str = "test (${{ matrix.leg }})";
pub(super) const TEST_TIMEOUT_MINUTES: i64 = 30;

pub(super) struct TestJob {
    pub(super) id: &'static str,
    pub(super) runner: &'static str,
    pub(super) step_env: [(&'static str, &'static str); 1],
}

pub(super) const TEST_JOBS: [TestJob; 2] = [
    TestJob {
        id: "test-linux",
        runner: "ubuntu-latest",
        step_env: [(TEMP_FOLDS_CASE_KEY, "")],
    },
    TestJob {
        id: "test-macos",
        runner: "macos-latest",
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
