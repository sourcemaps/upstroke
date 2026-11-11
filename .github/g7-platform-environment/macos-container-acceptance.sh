#!/usr/bin/env bash
# macos-container-acceptance.sh -- bounded native macOS Container acceptance
# capture (G7 M2, commission G7-M2-CONTAINER-ACCEPTANCE-20261109-1).
#
# A native Darwin coordinator -- this checkout's test binary, built and run on
# the macOS host -- drives the Docker Engine in the task-owned local Linux VM
# that the reviewed macos-docker-capability.sh probe started. Containers run in
# that VM, never natively on macOS.
#
#   build    before setup: native provenance, then
#            `cargo test --all-features --lib --no-run` of this checkout
#   capture  after the reviewed probe: require its CAPABILITY_OBSERVED verdict,
#            reuse the endpoint, client config and PATH it recorded, put every
#            product fixture under its shared G7_BIND_DIR, run the one ignored
#            test engine::tests::container_acceptance_record against the image
#            the probe pulled by digest, and check, decode and hash the
#            record's frame
#
# Cleanup stays the reviewed `macos-docker-capability.sh cleanup`; this script
# deletes nothing and starts or stops no VM. Every step's output and exit
# status is kept under $G7_LOG_DIR/container-acceptance. Nothing is retried and
# no failure is discarded: any failed check fails the step. No secret is read.
# Written for macOS /bin/bash 3.2.
set -uo pipefail

LOG_DIR=${G7_LOG_DIR:?G7_LOG_DIR must name the setup probe log directory}
OUT="$LOG_DIR/container-acceptance"
SELF_DIR=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$SELF_DIR/../.." && pwd)
BASELINE="$SELF_DIR/macos-docker-capability.sh"
TEST='engine::tests::container_acceptance_record'
MARKER='UPSTROKE-CONTAINER-ACCEPTANCE-RECORD'
# The reviewed probe's SMOKE_IMAGE, which it pulled by digest; checked below.
IMAGE='alpine:3.20@sha256:d9e853e87e55526f6b2917df91a2115c36dd7c696a35be12163d44e6e2a4b6bc'
EXPECT_VERSIONS='darwin/amd64 29.9.3 | linux/amd64 29.9.3'
BUILD_SECONDS=${G7_BUILD_SECONDS:-2700}
RUN_SECONDS=${G7_RUN_SECONDS:-1500}
mkdir -p "$OUT"
CHECKS="$OUT/checks.tsv"
STEPS="$OUT/steps.jsonl"
SEQ=0
[ -f "$STEPS" ] && SEQ=$(wc -l < "$STEPS" | tr -d ' ')
LAST_LOG=/dev/null
FAILED=0

now() { date -u +%Y-%m-%dT%H:%M:%SZ; }

# The reviewed probe's supervisor: the command in its own process group with a
# hard wall-clock limit, exit 124 on timeout, the group signalled only if the
# child verifiably leads it and only while the child is still unreaped.
# shellcheck disable=SC2016  # a Perl program, not shell
BOUNDED_PL='
use strict; use warnings; use POSIX qw(:sys_wait_h);
my $t = shift @ARGV;
my $pid = fork();
defined $pid or die "bounded: fork failed: $!\n";
if ($pid == 0) {
  setpgrp(0, 0) or do { print STDERR "bounded: setpgrp failed: $!\n"; POSIX::_exit(126) };
  exec { $ARGV[0] } @ARGV or do { print STDERR "bounded: exec failed: $!\n"; POSIX::_exit(127) };
}
my $deadline = time + $t;
while (time < $deadline) {
  if (waitpid($pid, WNOHANG) == $pid) { my $s = $?; exit(($s & 127) ? 128 + ($s & 127) : $s >> 8); }
  select(undef, undef, undef, 0.2);
}
my $grp = (getpgrp($pid) == $pid) ? "-" : "";
print STDERR "bounded: limit ${t}s reached; TERM then KILL to ", ($grp ? "process group" : "process"), " $pid\n";
kill "${grp}TERM", $pid;
sleep 5;
kill "${grp}KILL", $pid;
waitpid($pid, 0);
exit 124;
'
bounded() {
  local secs=$1
  shift
  if ! command -v perl >/dev/null 2>&1; then
    echo "bounded: perl is required for supervised timeouts; refusing to run unsupervised"
    return 125
  fi
  perl -e "$BOUNDED_PL" "$secs" "$@"
}

# step NAME TIMEOUT_SECONDS CMD... -- bounded, full output kept in its own log.
step() {
  local name=$1 secs=$2 f t0 rc
  shift 2
  SEQ=$((SEQ + 1))
  f=$(printf '%s/%02d-%s.log' "$OUT" "$SEQ" "$name")
  t0=$(date +%s)
  { printf '# %s step=%s timeout=%ss\n# $' "$(now)" "$name" "$secs"; printf ' %q' "$@"; printf '\n'; } > "$f"
  bounded "$secs" "$@" >> "$f" 2>&1
  rc=$?
  case $rc in 124|125|137|143) printf '# TIMEOUT, kill or unsupervised refusal (rc=%s); limit %ss\n' "$rc" "$secs" >> "$f" ;; esac
  printf '# %s exit=%s secs=%s\n' "$(now)" "$rc" "$(( $(date +%s) - t0 ))" >> "$f"
  printf '{"seq":%d,"step":"%s","rc":%d,"secs":%d,"log":"%s"}\n' \
    "$SEQ" "$name" "$rc" "$(( $(date +%s) - t0 ))" "$(basename "$f")" >> "$STEPS"
  LAST_LOG=$f
  return "$rc"
}

last_out() { grep -v '^# ' "$LAST_LOG"; }
b() { if [ "$1" = 0 ]; then echo 1; else echo 0; fi; }
both() { if [ "$1" = 0 ] && [ "$2" = 1 ]; then echo 1; else echo 0; fi; }

# check NAME 0|1 DETAIL -- a required row; FAIL makes the capture not established.
check() {
  local verdict=FAIL
  if [ "$2" = 1 ]; then verdict=PASS; else FAILED=$((FAILED + 1)); fi
  printf '%s\t%s\t%s\n' "$verdict" "$1" "$(printf '%s' "$3" | tr '\t\n' '  ' | cut -c1-300)" | tee -a "$CHECKS"
}

sha256_of() { shasum -a 256 "$1" | awk '{print $1}'; }

cmd_provenance() {
  local v
  echo "date_utc=$(now)"
  sw_vers || return 1
  uname -a
  printf 'uid=%s gid=%s user=%s groups=%s\n' "$(id -u)" "$(id -g)" "$(id -un)" "$(id -G)"
  for v in kern.hv_support hw.model machdep.cpu.brand_string kern.osversion; do
    printf '%s=%s\n' "$v" "$(sysctl -n "$v" 2>/dev/null || echo n/a)"
  done
  for v in ImageOS ImageVersion RUNNER_OS RUNNER_ARCH RUNNER_NAME RUNNER_ENVIRONMENT \
           GITHUB_REPOSITORY GITHUB_REF GITHUB_SHA GITHUB_RUN_ID GITHUB_RUN_ATTEMPT \
           GITHUB_JOB GITHUB_WORKFLOW GITHUB_EVENT_NAME G7_RUNNER_LABEL; do
    eval "printf '%s=%s\n' \"\$v\" \"\${$v-<unset>}\""
  done
  echo "--- toolchain"
  command -v cargo rustc rustup git || return 1
  rustup show active-toolchain || return 1
  rustc -vV || return 1
  cargo -V || return 1
  git --version || return 1
  echo "--- source"
  echo "repo=$REPO"
  git -C "$REPO" rev-parse HEAD 'HEAD^{tree}' || return 1
  echo "tracked_changes:"; git -C "$REPO" status --porcelain --untracked-files=no
  echo "--- scripts"
  shasum -a 256 "$0" "$BASELINE" "$SELF_DIR/lima-g7docker.yaml.in"
}

cmd_build() {
  local rc
  : > "$CHECKS"
  step provenance 120 bash "$0" _provenance; rc=$?
  check native_provenance_recorded "$(b $rc)" "rc=$rc $(grep -E '^(ProductVersion|BuildVersion):' "$LAST_LOG" | tr '\n' ' ')"
  step cargo-build "$BUILD_SECONDS" bash -c 'cd "$1" && cargo test --all-features --lib --no-run' _ "$REPO"; rc=$?
  check test_binary_built "$(b $rc)" "rc=$rc"
  [ "$FAILED" = 0 ]
}

cmd_capture() {
  local rc p summary tmp_root log frames record want bytes got
  summary="$LOG_DIR/SUMMARY.txt"
  p=0; [ -f "$summary" ] && head -1 "$summary" | grep -q '^verdict=CAPABILITY_OBSERVED failed_checks=0$' && p=1
  check setup_capability_observed "$p" "$(head -1 "$summary" 2>/dev/null || echo 'no SUMMARY.txt')"
  [ "$p" = 1 ] || return 1
  p=0; [ -f "$LOG_DIR/state.env" ] && p=1
  check setup_layout_recorded "$p" "$LOG_DIR/state.env"
  [ "$p" = 1 ] || return 1
  # shellcheck source=/dev/null  # written by the reviewed probe in this job: printf %q-quoted paths only
  . "$LOG_DIR/state.env"
  DOCKER_CONFIG="$WORK/docker-config"
  export LIMA_HOME DOCKER_CONFIG
  export PATH="$WORK/opt/lima/bin:$WORK/opt/docker:$PATH"
  { printf 'RUN_TAG=%s\nLIMA_HOME=%s\nBIND_DIR=%s\nWORK=%s\nINSTANCE=%s\nDOCKER_CONFIG=%s\n' \
      "$RUN_TAG" "$LIMA_HOME" "$BIND_DIR" "$WORK" "$INSTANCE" "$DOCKER_CONFIG"
    printf 'docker=%s\nlimactl=%s\n' "$(command -v docker)" "$(command -v limactl)"; } > "$OUT/layout.txt"
  p=0; grep -q -F "SMOKE_IMAGE='$IMAGE'" "$BASELINE" && p=1
  check image_is_the_setup_digest_pin "$p" "$IMAGE"

  step docker-context 30 docker context show; rc=$?
  p=0; [ "$(last_out)" = g7lima ] && p=1
  check endpoint_is_the_setup_context "$(both $rc $p)" "rc=$rc context=$(last_out | tr '\n' ' ')"
  step docker-endpoint 30 docker context inspect g7lima --format '{{json .Endpoints.docker}}'; rc=$?
  p=0; last_out | grep -q -F "unix://$LIMA_HOME/$INSTANCE/sock/docker.sock" && p=1
  check endpoint_is_the_local_vm_socket "$(both $rc $p)" "rc=$rc $(last_out | tr '\n' ' ')"
  step docker-versions 60 docker version --format '{{.Client.Os}}/{{.Client.Arch}} {{.Client.Version}} | {{.Server.Os}}/{{.Server.Arch}} {{.Server.Version}}'; rc=$?
  p=0; [ "$(last_out)" = "$EXPECT_VERSIONS" ] && p=1
  check client_darwin_server_linux "$(both $rc $p)" "rc=$rc $(last_out | tr '\n' ' ')"
  step docker-info 60 docker info --format 'Name={{.Name}} OSType={{.OSType}} OperatingSystem={{.OperatingSystem}} KernelVersion={{.KernelVersion}} Architecture={{.Architecture}}'; rc=$?
  p=0; last_out | grep -q ' OSType=linux ' && p=1
  check daemon_is_linux "$(both $rc $p)" "rc=$rc $(last_out | tr '\n' ' ')"
  step image-present 60 docker image inspect "$IMAGE" --format '{{.Id}} {{json .RepoDigests}} {{.Os}}/{{.Architecture}}'; rc=$?
  check image_present_without_pull "$(b $rc)" "rc=$rc $(last_out | tr '\n' ' ')"

  tmp_root="$BIND_DIR/container-acceptance-tmp"
  mkdir -p "$tmp_root"
  step shared-fixture-root 60 limactl shell --workdir / "$INSTANCE" test -d "$tmp_root"; rc=$?
  check fixture_root_is_shared_into_the_vm "$(b $rc)" "rc=$rc $tmp_root"
  [ "$FAILED" = 0 ] || return 1

  step acceptance-run "$RUN_SECONDS" env TMPDIR="$tmp_root" UPSTROKE_CONTAINER_ACCEPTANCE_IMAGE="$IMAGE" \
    UPSTROKE_TEST_TEMP_FOLDS_CASE=1 bash -c 'cd "$1" && cargo test --all-features --lib "$2" -- --ignored --exact --nocapture' \
    _ "$REPO" "$TEST"; rc=$?
  log=$LAST_LOG
  check acceptance_test_exit_zero "$(b $rc)" "rc=$rc log=$(basename "$log")"
  p=0; grep -qx "test $TEST ... ok" "$log" && grep -q '^test result: ok\. 1 passed; 0 failed; 0 ignored; ' "$log" && p=1
  check exactly_the_one_acceptance_test_passed "$p" "$(grep -E '^test result:' "$log" | tr '\n' ' ')"
  frames="$OUT/frame.txt"
  grep "^$MARKER " "$log" > "$frames"
  awk -v marker="$MARKER" '
    $1 == marker { f[++m] = $0 }
    END {
      nb = split(f[1], b, " "); ne = split(f[m], e, " ")
      ok = m >= 3 && nb == 7 && ne == 7 && b[2] == "v1" && b[3] == "BEGIN" && e[2] == "v1" && e[3] == "END"
      ok = ok && b[4] " " b[5] " " b[6] " " b[7] == e[4] " " e[5] " " e[6] " " e[7] && b[5] "" == (m - 2) ""
      for (k = 2; ok && k < m; k++) ok = split(f[k], c, " ") == 5 && c[2] == "v1" && c[3] "" == b[4] "" && c[4] "" == (k - 1) "" && length(c[5]) == (k < m - 1 ? 192 : 2 * (b[6] - (b[5] - 1) * 96))
      exit !ok
    }' "$frames"; rc=$?
  check record_frame_complete "$(b $rc)" "lines=$(wc -l < "$frames" | tr -d ' ')"
  if [ "$rc" = 0 ]; then
    record="$OUT/record.json"
    awk '$3 != "BEGIN" && $3 != "END" { print $5 }' "$frames" | tr -d '\n' | xxd -r -p > "$record"
    want=$(head -1 "$frames" | awk '{print $7}')
    bytes=$(head -1 "$frames" | awk '{print $6}')
    got=$(sha256_of "$record")
    p=0; [ "$got" = "$want" ] && [ "$(wc -c < "$record" | tr -d ' ')" = "$bytes" ] && p=1
    check record_decodes_to_its_digest "$p" "sha256=$got want=$want bytes=$bytes"
    printf '%s  record.json\n' "$got" > "$OUT/record.sha256"
  fi
  step leftover-labelled-containers 60 docker ps -a --filter label=upstroke.private_root --format '{{.Names}} {{.Status}}'; rc=$?
  p=0; [ -z "$(last_out)" ] && p=1
  check no_labelled_container_left "$(both $rc $p)" "rc=$rc $(last_out | tr '\n' ' ')"
  [ "$FAILED" = 0 ]
}

summarize() {
  {
    echo "verdict=$1 failed_checks=$FAILED"
    echo "--- checks (PASS/FAIL decide)"; cat "$CHECKS" 2>/dev/null
    echo "--- not claimed here: Gate7 or M2 acceptance (root judges the decoded record), native macOS"
    echo "    container execution (containers run in the local Linux VM), kills, crashes or cleanup"
  } | tee "$OUT/SUMMARY-$2.txt"
}

case ${1:-} in
  build)
    if cmd_build; then summarize BUILT build; exit 0; fi
    summarize BUILD_FAILED build; exit 1 ;;
  capture)
    if cmd_capture; then summarize CAPTURED capture; exit 0; fi
    summarize CAPTURE_NOT_ESTABLISHED capture; exit 1 ;;
  _provenance) cmd_provenance ;;
  *) sed -n '2,25p' "$0"; exit 2 ;;
esac
