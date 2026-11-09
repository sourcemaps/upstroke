#!/usr/bin/env bash
# macos-container-acceptance-selftest.sh -- regression fixture for
# macos-container-acceptance.sh's initialization and dispatch (G7-M2-MACOS-001).
#
# Runs the real script, unmodified, with stub cargo, rustc, rustup, sw_vers and
# docker (and shasum only where the host has none) first on PATH, a scratch
# G7_LOG_DIR per case and a hard per-run bound. It builds nothing and contacts no
# Docker daemon or VM. Run by hand; no workflow runs it.
#
#   G7_SELFTEST_SHELL   shell that runs the target and its nested `bash`
#                       (default: bash on PATH; name /bin/bash on macOS for 3.2)
#   G7_SELFTEST_TARGET  script under test (default: the sibling script)
#   G7_SELFTEST_WORK    parent directory for this run's work directory
#
# Cases, each a verdict the target must reach from its own checks:
#   build-ok        stub cargo test exits 0 -> exit 0, verdict=BUILT, both rows PASS
#   build-fail      stub cargo test exits 1 -> exit 1, verdict=BUILD_FAILED, no BUILT
#   capture-unset   no setup verdict        -> exit 1, CAPTURE_NOT_ESTABLISHED,
#                                              and docker is never invoked
#   log-dir-unset   G7_LOG_DIR unset        -> non-zero, with the exact diagnostic
# Exits 0 only if every case holds. Written for macOS /bin/bash 3.2.
set -uo pipefail

SELF_DIR=$(cd "$(dirname "$0")" && pwd)
TARGET=${G7_SELFTEST_TARGET:-$SELF_DIR/macos-container-acceptance.sh}
SHELL_NAME=${G7_SELFTEST_SHELL:-bash}
SHELL_PATH=$(command -v "$SHELL_NAME") || { echo "selftest: no shell $SHELL_NAME"; exit 2; }
WORK=$(mktemp -d "${G7_SELFTEST_WORK:-${TMPDIR:-/tmp}}/g7-macos-selftest.XXXXXX") || exit 2
STUBS="$WORK/stubs"
mkdir -p "$STUBS"
FAILED=0
BOUND=120

stub() {
  printf '#!/bin/sh\n%s\n' "$2" > "$STUBS/$1"
  chmod 0755 "$STUBS/$1"
}
stub bash "exec '$SHELL_PATH' \"\$@\""
stub cargo 'if [ "${1:-}" = -V ]; then echo "cargo 0.0.0-selftest-stub"; exit 0; fi
echo "$*" >> "$G7_SELFTEST_CALLS/cargo"
exit "${G7_STUB_CARGO_RC:-0}"'
stub rustc 'echo "rustc 0.0.0-selftest-stub"; echo "host: x86_64-apple-darwin"'
stub rustup 'echo "selftest-stub-toolchain (selftest)"'
stub sw_vers 'echo "ProductName: macOS"; echo "ProductVersion: 0.0-selftest"; echo "BuildVersion: 0selftest"'
stub docker 'echo "$*" >> "$G7_SELFTEST_CALLS/docker"; exit 99'
if ! command -v shasum >/dev/null 2>&1; then
  stub shasum 'shift 2; sha256sum "$@"'
fi

# run CASE SUBCOMMAND [ENV-OPTION-OR-ASSIGNMENT]... -- the target under the selftest
# shell, bounded, with its own log dir and call records. Env options (-u NAME) and
# NAME=VALUE pairs go to env ahead of PATH, so an option precedes every assignment.
run() {
  local case=$1 subcommand=$2
  shift 2
  mkdir -p "$WORK/$case/log" "$WORK/$case/calls"
  env "$@" PATH="$STUBS:$PATH" G7_SELFTEST_CALLS="$WORK/$case/calls" \
    perl -e 'alarm shift @ARGV; exec @ARGV or die "exec: $!\n"' "$BOUND" "$SHELL_PATH" "$TARGET" "$subcommand" \
    > "$WORK/$case/stdout.txt" 2> "$WORK/$case/stderr.txt"
}

expect() {
  if [ "$2" = 1 ]; then echo "PASS  $1"; else echo "FAIL  $1  ($3)"; FAILED=$((FAILED + 1)); fi
}

first_line() { head -n 1 "$1" 2>/dev/null || true; }
has_row() { grep -q "^$1	$2	" "$3" 2>/dev/null && echo 1 || echo 0; }

echo "selftest target=$TARGET"
echo "selftest shell=$SHELL_PATH ($("$SHELL_PATH" --version 2>/dev/null | head -n 1))"
echo "selftest work=$WORK"

run build-ok build G7_LOG_DIR="$WORK/build-ok/log" G7_STUB_CARGO_RC=0
rc=$?
out="$WORK/build-ok/log/container-acceptance"
expect "build-ok exit 0" "$([ "$rc" = 0 ] && echo 1 || echo 0)" "rc=$rc"
expect "build-ok verdict BUILT" "$([ "$(first_line "$out/SUMMARY-build.txt")" = 'verdict=BUILT failed_checks=0' ] && echo 1 || echo 0)" "$(first_line "$out/SUMMARY-build.txt")"
expect "build-ok provenance row PASS" "$(has_row PASS native_provenance_recorded "$out/checks.tsv")" "checks.tsv"
expect "build-ok build row PASS" "$(has_row PASS test_binary_built "$out/checks.tsv")" "checks.tsv"
expect "build-ok ran the stub cargo test" "$(grep -q -x 'test --all-features --lib --no-run' "$WORK/build-ok/calls/cargo" 2>/dev/null && echo 1 || echo 0)" "calls/cargo"

run build-fail build G7_LOG_DIR="$WORK/build-fail/log" G7_STUB_CARGO_RC=1
rc=$?
out="$WORK/build-fail/log/container-acceptance"
expect "build-fail exit 1" "$([ "$rc" = 1 ] && echo 1 || echo 0)" "rc=$rc"
expect "build-fail verdict BUILD_FAILED" "$([ "$(first_line "$out/SUMMARY-build.txt")" = 'verdict=BUILD_FAILED failed_checks=1' ] && echo 1 || echo 0)" "$(first_line "$out/SUMMARY-build.txt")"
expect "build-fail build row FAIL" "$(has_row FAIL test_binary_built "$out/checks.tsv")" "checks.tsv"
expect "build-fail never says BUILT" "$(grep -q '^verdict=BUILT ' "$out"/SUMMARY-*.txt 2>/dev/null && echo 0 || echo 1)" "a BUILT verdict was written"

run capture-unset capture G7_LOG_DIR="$WORK/capture-unset/log"
rc=$?
out="$WORK/capture-unset/log/container-acceptance"
expect "capture-unset exit 1" "$([ "$rc" = 1 ] && echo 1 || echo 0)" "rc=$rc"
expect "capture-unset verdict CAPTURE_NOT_ESTABLISHED" "$([ "$(first_line "$out/SUMMARY-capture.txt")" = 'verdict=CAPTURE_NOT_ESTABLISHED failed_checks=1' ] && echo 1 || echo 0)" "$(first_line "$out/SUMMARY-capture.txt")"
expect "capture-unset setup row FAIL" "$(has_row FAIL setup_capability_observed "$out/checks.tsv")" "checks.tsv"
expect "capture-unset never reached docker" "$([ -e "$WORK/capture-unset/calls/docker" ] && echo 0 || echo 1)" "docker was invoked"

run log-dir-unset build -u G7_LOG_DIR
rc=$?
expect "log-dir-unset exit non-zero" "$([ "$rc" != 0 ] && echo 1 || echo 0)" "rc=$rc"
expect "log-dir-unset exact diagnostic" "$(grep -q -F 'G7_LOG_DIR: G7_LOG_DIR must name the setup probe log directory' "$WORK/log-dir-unset/stderr.txt" && echo 1 || echo 0)" "$(head -c 200 "$WORK/log-dir-unset/stderr.txt")"

if [ "$FAILED" = 0 ]; then echo "selftest=PASS failed=0 shell=$SHELL_PATH"; exit 0; fi
echo "selftest=FAIL failed=$FAILED shell=$SHELL_PATH"
exit 1
