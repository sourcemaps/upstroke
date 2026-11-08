#!/usr/bin/env bash
# macos-docker-capability.sh -- bounded macOS Docker capability probe
# (G7-PLATFORM-ENVIRONMENT-SETUP-20261108-1).
#
# Ordinary environment-capability evidence on a standard public GitHub-hosted
# macOS Intel runner: the native macOS host drives a digest-verified Docker CLI
# against Docker Engine inside a task-owned local Linux VM (Lima). This is NOT
# Gate7 acceptance, not a required check, and asserts nothing about the product.
#
#   probe    host identity -> digest-verified tools -> local VM (vz first when
#            kern.hv_support is 1, then QEMU; the pinned Lima 2.2.1 runs QEMU
#            with TCG software emulation only when kern.hv_support is not 1 and
#            does not retry TCG after an HVF failure) -> Docker client and
#            daemon identity -> execution location -> one disposable container
#            -> same-path read/write bind mount
#   cleanup  bounded, verified teardown of what probe created; exits non-zero
#            (CLEANUP.txt says INCOMPLETE) when teardown cannot be confirmed
#
# Every step's output and exit status is kept under $G7_LOG_DIR. Nothing is
# retried silently and no failure is discarded. No secret is read; only an
# allowlist of runner variables is recorded. Written for macOS /bin/bash 3.2.
set -uo pipefail

# ---------------------------------------------------------------- pins ------
# Lima and the Docker CLI are extracted and run only after their SHA-256
# digests match. QEMU (only when vz is unusable) comes from Homebrew with
# auto-update off, and the VM's transitive apt dependencies are resolved by
# apt: both are recorded, not pinned (accepted for this ordinary probe).
# Lima v2.2.1 contains lima-vm/lima#5137 (merge 2a9cb9a4, 2026-06-22).
LIMA_VERSION=2.2.1
LIMA_URL="https://github.com/lima-vm/lima/releases/download/v${LIMA_VERSION}/lima-${LIMA_VERSION}-Darwin-x86_64.tar.gz"
LIMA_SHA256=6b776e4bc41f358e6ee01fefcc78e2eb26f11635a0fcff632234af77a566c6c4
DOCKER_CLI_VERSION=29.9.3
DOCKER_CLI_URL="https://download.docker.com/mac/static/stable/x86_64/docker-${DOCKER_CLI_VERSION}.tgz"
DOCKER_CLI_SHA256=f7d8f835391cd0a8c676d9a8d2547f909da6a2d029595943b0ce8d5f169abe47
# alpine:3.20 OCI index digest (linux/amd64 manifest c64c687c...), 2026-11-08.
SMOKE_IMAGE='alpine:3.20@sha256:d9e853e87e55526f6b2917df91a2115c36dd7c696a35be12163d44e6e2a4b6bc'
EXPECT_CLIENT="darwin/amd64 ${DOCKER_CLI_VERSION}"
EXPECT_SERVER="linux/amd64 29.9.3"   # docker-ce pinned in lima-g7docker.yaml.in

# --------------------------------------------------------------- layout -----
SELF="$0"
SELF_DIR=$(cd "$(dirname "$0")" && pwd)
TEMPLATE="$SELF_DIR/lima-g7docker.yaml.in"
LOG_DIR=${G7_LOG_DIR:?G7_LOG_DIR must name the log directory}
RUN_TAG="${GITHUB_RUN_ID:-local}-${GITHUB_RUN_ATTEMPT:-0}"
WORK=${G7_WORK_DIR:-${RUNNER_TEMP:-${TMPDIR:-/tmp}}/g7-macos-docker-work}
# Short on purpose: macOS limits unix socket paths to 104 bytes.
LIMA_HOME=${G7_LIMA_HOME:-$HOME/.g7lima}
INSTANCE=g7d
BIND_DIR=${G7_BIND_DIR:-$HOME/g7plat-bind-$RUN_TAG}
OPT="$WORK/opt"
DOCKER_CONFIG="$WORK/docker-config"
export LIMA_HOME DOCKER_CONFIG
export PATH="$OPT/lima/bin:$OPT/docker:$PATH"
export HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_NO_INSTALL_CLEANUP=1 HOMEBREW_NO_ENV_HINTS=1 HOMEBREW_NO_ANALYTICS=1

# ---------------------------------------------------------------- budgets ---
# One shared deadline per phase clamps every expensive step (bstep below).
BUDGET_SECONDS=${G7_BUDGET_SECONDS:-2160}                  # probe; the workflow step allows 38 min
CLEANUP_BUDGET_SECONDS=${G7_CLEANUP_BUDGET_SECONDS:-200}   # cleanup; the workflow step allows 240 s
POST_START_SECONDS=420   # kept back at VM start for identity, container, bind mount and logs
FINAL_RESERVE=45         # never handed to a step: summary and log capture
VZ_START_MAX=900         # a vz attempt that still has a QEMU fallback behind it gets at most this
BREW_QEMU_MAX=600        # fallback install; attempted only when install + start + post-start still fit
MIN_START=300
MIN_STEP=10
DEADLINE=0

STEPS="$LOG_DIR/steps.jsonl"
CHECKS="$LOG_DIR/checks.tsv"
mkdir -p "$LOG_DIR" "$WORK"

# -------------------------------------------------------------- helpers -----
now() { date -u +%Y-%m-%dT%H:%M:%SZ; }
SEQ=0
[ -f "$STEPS" ] && SEQ=$(wc -l < "$STEPS" | tr -d ' ')
LAST_LOG=/dev/null
FAILED=0

# Supervisor: run the command in its own process group with a hard wall-clock
# limit; exit 124 on timeout. Not alarm+exec, because Go programs (limactl,
# docker) ignore SIGALRM. The child stays unreaped until after the final group
# signal, so its PID and the group ID it leads cannot be reused by an
# unrelated process while we signal; the group is signalled only if the child
# verifiably leads it.
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

# step NAME TIMEOUT_SECONDS CMD...  -- bounded, full output kept in its own log.
step() {
  local name=$1 secs=$2 f t0 rc
  shift 2
  SEQ=$((SEQ + 1))
  f=$(printf '%s/%02d-%s.log' "$LOG_DIR" "$SEQ" "$name")
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

# bstep NAME MAX_SECONDS CMD... -- step clamped to the shared DEADLINE minus
# FINAL_RESERVE; skipped with rc 124 when too little time remains.
bstep() {
  local name=$1 max=$2 left
  shift 2
  left=$(( DEADLINE - $(date +%s) - FINAL_RESERVE ))
  if [ "$left" -lt "$MIN_STEP" ]; then
    SEQ=$((SEQ + 1))
    LAST_LOG=$(printf '%s/%02d-%s.log' "$LOG_DIR" "$SEQ" "$name")
    printf '# %s step=%s SKIPPED: shared deadline reached (%ss left)\n' "$(now)" "$name" "$left" > "$LAST_LOG"
    printf '{"seq":%d,"step":"%s","rc":124,"secs":0,"log":"%s","skipped":"deadline"}\n' \
      "$SEQ" "$name" "$(basename "$LAST_LOG")" >> "$STEPS"
    return 124
  fi
  [ "$max" -gt "$left" ] && max=$left
  step "$name" "$max" "$@"
}

# Output of the last step without its '# ' framing lines.
last_out() { grep -v '^# ' "$LAST_LOG"; }

# b RC -> 1 when RC is 0, else 0.   both RC PRED -> 1 only when RC is 0 and PRED is 1.
b() { if [ "$1" = 0 ]; then echo 1; else echo 0; fi; }
both() { if [ "$1" = 0 ] && [ "$2" = 1 ]; then echo 1; else echo 0; fi; }

# check NAME 0|1 DETAIL -- a required row; FAIL makes the verdict not established.
check() {
  local verdict=FAIL
  if [ "$2" = 1 ]; then verdict=PASS; else FAILED=$((FAILED + 1)); fi
  printf '%s\t%s\t%s\n' "$verdict" "$1" "$(printf '%s' "$3" | tr '\t\n' '  ' | cut -c1-300)" | tee -a "$CHECKS"
}

# attempt NAME DETAIL -- kept as evidence, never decides the verdict (e.g. vz
# failing before a QEMU fallback succeeds).
attempt() {
  printf 'ATTEMPT\t%s\t%s\n' "$1" "$(printf '%s' "$2" | tr '\t\n' '  ' | cut -c1-300)" | tee -a "$CHECKS"
}

# diag NAME RC -- a diagnostic step explicitly allowed to fail.
diag() {
  printf 'DIAG\t%s\trc=%s (diagnostic; does not decide the verdict)\n' "$1" "$2" >> "$CHECKS"
}

sha256_of() {
  if command -v shasum >/dev/null 2>&1; then shasum -a 256 "$1" | awk '{print $1}'
  else openssl dgst -sha256 -r "$1" | awk '{print $1}'; fi
}

# --------------------------------------------------- internal subcommands ---
# fetch URL SHA256 DEST: download to DEST.part and publish DEST only after the
# digest matches. On any failure the owned .part file is removed; if the
# supervisor kills the transfer, the .part file stays behind but is never
# extracted (only DEST is, and only after a zero fetch status).
cmd_fetch() {
  local url=$1 want=$2 dest=$3 got part="$3.part"
  rm -f "$part" "$dest"
  if ! curl -fsSL --retry 3 --retry-delay 5 --max-time 600 -o "$part" "$url"; then
    echo "download failed: $url"; rm -f "$part"; return 1
  fi
  got=$(sha256_of "$part")
  echo "url=$url"; echo "sha256=$got"; echo "expected=$want"; echo "bytes=$(wc -c < "$part" | tr -d ' ')"
  if [ "$got" != "$want" ]; then echo "DIGEST MISMATCH: refusing $url"; rm -f "$part"; return 1; fi
  mv -f "$part" "$dest" && echo "digest OK; admitted as $dest"
}

cmd_host_identity() {
  local k v
  echo "date_utc=$(now)"
  sw_vers || return 1
  uname -a
  for k in hw.model hw.ncpu hw.memsize machdep.cpu.brand_string kern.hv_support kern.hv_vmm_present kern.osversion; do
    printf '%s=%s\n' "$k" "$(sysctl -n "$k" 2>/dev/null || echo n/a)"
  done
  # Hardware profile without serial numbers or hardware/provisioning UUIDs.
  system_profiler SPHardwareDataType 2>/dev/null | grep -v -i -E 'serial|uuid|udid'
  for v in ImageOS ImageVersion RUNNER_OS RUNNER_ARCH RUNNER_NAME RUNNER_ENVIRONMENT \
           GITHUB_REPOSITORY GITHUB_REF GITHUB_SHA GITHUB_RUN_ID GITHUB_RUN_ATTEMPT \
           GITHUB_JOB GITHUB_WORKFLOW GITHUB_EVENT_NAME G7_RUNNER_LABEL; do
    eval "printf '%s=%s\n' \"\$v\" \"\${$v-<unset>}\""
  done
  df -h / "$HOME"
  vm_stat | head -6
  echo "--- preinstalled container tooling (expected none)"
  for k in docker limactl colima podman qemu-system-x86_64; do printf '%s: %s\n' "$k" "$(command -v "$k" || echo absent)"; done
}

# QEMU identity, whether preinstalled or installed by this probe.
cmd_qemu_identity() {
  echo "qemu-system-x86_64=$(command -v qemu-system-x86_64 || echo absent)"
  qemu-system-x86_64 --version || return 1
  echo "brew=$(brew --version 2>/dev/null | head -1) HOMEBREW_NO_AUTO_UPDATE=${HOMEBREW_NO_AUTO_UPDATE}"
  brew list --versions qemu 2>/dev/null || echo "qemu is not a Homebrew keg here"
  brew info --json=v2 qemu 2>/dev/null | jq -c '.formulae[0] | {name, stable: .versions.stable, revision, tap, installed: [.installed[] | {version, poured_from_bottle, built_as_bottle, installed_as_dependency}], bottle: .bottle.stable}' \
    || echo "brew info unavailable"
}

# render VMTYPE MOUNTTYPE OUT: fill the template with validated values only.
cmd_render() {
  local vmtype=$1 mounttype=$2 out=$3 user
  user=$(id -un)
  case $vmtype in vz|qemu) ;; *) echo "bad vmType $vmtype"; return 1 ;; esac
  case $mounttype in virtiofs|9p|reverse-sshfs) ;; *) echo "bad mountType $mounttype"; return 1 ;; esac
  case $BIND_DIR in /*) ;; *) echo "bind dir must be absolute"; return 1 ;; esac
  case "$BIND_DIR$user" in *[!A-Za-z0-9._/-]*) echo "unexpected characters in bind dir or user"; return 1 ;; esac
  sed -e "s|@VMTYPE@|$vmtype|g" -e "s|@MOUNTTYPE@|$mounttype|g" \
      -e "s|@BIND_DIR@|$BIND_DIR|g" -e "s|@SOCKET_USER@|$user|g" "$TEMPLATE" > "$out" || return 1
  if grep -n '@[A-Z_]*@' "$out"; then echo "unrendered placeholder"; return 1; fi
  echo "rendered $out (vmType=$vmtype mountType=$mounttype bind=$BIND_DIR socketUser=$user)"
  cat "$out"
}

# Copy instance logs (never the Lima _config directory, which holds its ssh key).
cmd_collect_instance_logs() {
  local dest=$1 d="$LIMA_HOME/$INSTANCE" f
  mkdir -p "$dest"
  [ -d "$d" ] || { echo "no instance dir $d"; return 0; }
  for f in "$d"/*.log "$d"/lima.yaml "$d"/serial*; do
    [ -f "$f" ] && cp "$f" "$dest/" && echo "copied $(basename "$f")"
  done
  return 0
}

cmd_vm_identity() {
  # shellcheck disable=SC2016  # the script runs in the guest; its $ expansions belong to the guest shell
  limactl shell --workdir / "$INSTANCE" bash -c '
    echo "hostname=$(hostname)"; uname -a; . /etc/os-release; echo "os=$PRETTY_NAME"
    echo "virt=$(systemd-detect-virt 2>/dev/null || echo unknown)"; echo "nproc=$(nproc)"
    free -m | head -2; df -h / | tail -1; ip -4 -br addr
    grep -m1 "model name" /proc/cpuinfo; sudo dmesg 2>/dev/null | grep -i -m3 -E "hypervisor detected|tcg|hvf" || true
    echo "--- docker in guest"; sudo docker version --format "{{.Server.Os}}/{{.Server.Arch}} {{.Server.Version}}"
    sudo ls -l /var/run/docker.sock; findmnt -T "'"$BIND_DIR"'" -o TARGET,SOURCE,FSTYPE,OPTIONS || true
    dpkg-query -W -f="\${Package} \${Version}\n" docker-ce docker-ce-cli containerd.io'
}

cmd_location() {
  local sock="$LIMA_HOME/$INSTANCE/sock/docker.sock" rc=0
  echo "--- client endpoint"; docker context inspect g7lima --format '{{json .Endpoints.docker}}' || rc=1
  echo "--- forwarded socket on this host"; ls -l "$sock" || rc=1
  [ -S "$sock" ] || { echo "not a socket: $sock"; rc=1; }
  echo "--- VM processes on this host (Lima hostagent and hypervisor)"
  # shellcheck disable=SC2009  # full command lines with ppid/etime are the evidence wanted here
  ps -axo pid,ppid,etime,command | grep -E '[l]imactl hostagent|[q]emu-system|[V]irtualization\.VirtualMachine' | cut -c1-400
  echo "--- accelerator evidence (QEMU only; vz uses Virtualization.framework)"
  # shellcheck disable=SC2009  # the QEMU argv itself is the evidence
  ps -axo command | grep -E '[q]emu-system' | tr ' ' '\n' | grep -A1 -E '^-accel$|^-machine$' | tr '\n' ' '; echo
  grep -h -i -E 'hv_support|hvf|tcg|accel' "$LIMA_HOME/$INSTANCE"/*.log 2>/dev/null | tail -20 || true
  return "$rc"
}

# Exit 0 only if this instance is reported Stopped and no process references
# its directory.
cmd_instance_stopped() {
  local d="$LIMA_HOME/$INSTANCE" st pf p alive=0 procs
  st=$(limactl list --format '{{.Status}}' "$INSTANCE" 2>/dev/null || echo unknown)
  echo "limactl status=$st"
  for pf in "$d/ha.pid" "$d/qemu.pid" "$d/vz.pid"; do
    [ -f "$pf" ] || continue
    p=$(cat "$pf")
    if kill -0 "$p" 2>/dev/null; then echo "alive: $pf pid=$p"; alive=1; else echo "not running: $pf pid=$p"; fi
  done
  # shellcheck disable=SC2009  # match on the instance directory in full command lines
  procs=$(ps -axo pid,command | grep -F "$d" | grep -v -F 'grep' || true)
  if [ -n "$procs" ]; then echo "processes referencing $d:"; echo "$procs"; alive=1; fi
  [ "$st" = Stopped ] && [ "$alive" = 0 ]
}

# Remove only this probe's own paths; refuse anything else. Exit non-zero on
# any refusal or failed removal.
cmd_safe_rm() {
  local p bad=0
  for p in "$@"; do
    case $p in
      "$HOME"/.g7lima|"$HOME"/g7plat-bind-*|"$WORK"/opt|"$WORK"/dl|"$WORK"/docker-config) ;;
      *)
        if [ -z "${G7_LIMA_HOME:-}" ] || [ "$p" != "$G7_LIMA_HOME" ]; then
          echo "refusing to remove unexpected path: $p"; bad=1; continue
        fi ;;
    esac
    case $p in ""|/|"$HOME"|"$HOME"/) echo "refusing: $p"; bad=1; continue ;; esac
    if [ ! -e "$p" ]; then echo "already absent: $p"; continue; fi
    if rm -rf -- "$p" && [ ! -e "$p" ]; then echo "removed $p"; else echo "FAILED to remove $p"; bad=1; fi
  done
  return "$bad"
}

# Exit non-zero if any named path, or any process referencing the instance
# directory, is still present.
cmd_verify_gone() {
  local p bad=0 procs
  for p in "$@"; do
    if [ -e "$p" ]; then echo "STILL PRESENT: $p"; bad=1; else echo "absent: $p"; fi
  done
  # shellcheck disable=SC2009  # match on the instance directory in full command lines
  procs=$(ps -axo pid,command | grep -F "$LIMA_HOME/$INSTANCE" | grep -v -F 'grep' || true)
  if [ -n "$procs" ]; then echo "VM processes still present:"; echo "$procs"; bad=1
  else echo "no process references $LIMA_HOME/$INSTANCE"; fi
  return "$bad"
}

# --------------------------------------------------------------- probe ------
cmd_probe() {
  local t_start vmtypes vt mt="" started=0 cfg rem left need ncand i marker reads back rc rc2 p vm_host=""
  t_start=$(date +%s); DEADLINE=$(( t_start + BUDGET_SECONDS ))
  : > "$CHECKS"
  printf 'RUN_TAG=%q\nLIMA_HOME=%q\nBIND_DIR=%q\nWORK=%q\nINSTANCE=%q\n' "$RUN_TAG" "$LIMA_HOME" "$BIND_DIR" "$WORK" "$INSTANCE" > "$LOG_DIR/state.env"

  bstep host-identity 120 bash "$SELF" _host_identity; rc=$?
  p=0; grep -q '^ProductVersion:' "$LAST_LOG" && grep -q '^kern.hv_support=' "$LAST_LOG" && p=1
  check host_identity_recorded "$(both $rc $p)" "$(grep -E '^(ProductVersion|kern.hv_support|kern.hv_vmm_present|hw.model)' "$LAST_LOG" | tr '\n' ' ')"

  # Tool admission: both archives must verify before either is extracted, and
  # extraction must succeed before any extracted tool runs.
  mkdir -p "$WORK/dl" "$OPT/lima"
  bstep fetch-lima 660 bash "$SELF" _fetch "$LIMA_URL" "$LIMA_SHA256" "$WORK/dl/lima.tar.gz"; rc=$?
  check lima_download_verified "$(b $rc)" "lima ${LIMA_VERSION} sha256 ${LIMA_SHA256} fetch_rc=$rc"
  bstep fetch-docker-cli 660 bash "$SELF" _fetch "$DOCKER_CLI_URL" "$DOCKER_CLI_SHA256" "$WORK/dl/docker.tgz"; rc2=$?
  check docker_cli_download_verified "$(b $rc2)" "docker ${DOCKER_CLI_VERSION} sha256 ${DOCKER_CLI_SHA256} fetch_rc=$rc2"
  if [ "$rc" != 0 ] || [ "$rc2" != 0 ]; then
    check verified_tools_admitted 0 "a download did not verify; nothing was extracted or executed"; return 1
  fi
  # shellcheck disable=SC2016  # positional parameters of the inner bash, passed after "_"
  bstep unpack-tools 120 bash -c 'tar -xzf "$1" -C "$2/lima" && tar -xzf "$3" -C "$2" && ls -l "$2/lima/bin" "$2/docker"' _ "$WORK/dl/lima.tar.gz" "$OPT" "$WORK/dl/docker.tgz"; rc=$?
  p=0; [ -x "$OPT/lima/bin/limactl" ] && [ -x "$OPT/docker/docker" ] && p=1
  check verified_tools_admitted "$(both $rc $p)" "unpack_rc=$rc; limactl and docker present from the verified archives"
  [ "$(both $rc $p)" = 1 ] || return 1
  # shellcheck disable=SC2016  # expanded by the inner bash, which inherits PATH
  bstep tool-identity 60 bash -c 'limactl --version; docker --version; file "$(command -v limactl)" "$(command -v docker)"; codesign -dv --entitlements - "$(command -v limactl)" 2>&1 | grep -E "Identifier|Signature|virtualization|hypervisor" || true'
  diag tool-identity $?

  # vz needs Hypervisor.framework; QEMU is tried after it, or alone without it.
  case ${G7_VMTYPES:-auto} in
    auto) if [ "$(sysctl -n kern.hv_support 2>/dev/null || echo 0)" = 1 ]; then vmtypes="vz qemu"; else vmtypes="qemu"; fi ;;
    *) vmtypes=$G7_VMTYPES ;;
  esac
  echo "vmType order: $vmtypes (kern.hv_support=$(sysctl -n kern.hv_support 2>/dev/null || echo n/a))" | tee "$LOG_DIR/vmtype-order.txt"
  mkdir -p "$BIND_DIR"
  ncand=$(echo "$vmtypes" | wc -w | tr -d ' '); i=0
  for vt in $vmtypes; do
    i=$((i + 1))
    if [ "$vt" = vz ]; then mt=virtiofs; else mt=9p; fi
    if [ "$vt" = qemu ]; then
      if ! command -v qemu-system-x86_64 >/dev/null 2>&1; then
        need=$(( BREW_QEMU_MAX + MIN_START + POST_START_SECONDS + FINAL_RESERVE ))
        left=$(( DEADLINE - $(date +%s) ))
        if [ "$left" -lt "$need" ]; then
          attempt vm_start_qemu "skipped: ${left}s left, ${need}s needed for brew install + start + post-start"; continue
        fi
        bstep brew-install-qemu "$BREW_QEMU_MAX" brew install qemu; rc=$?
        [ "$rc" = 0 ] || attempt brew_install_qemu "FAILED rc=$rc (see $(basename "$LAST_LOG"))"
      fi
      bstep qemu-identity 90 bash "$SELF" _qemu_identity; diag qemu-identity $?
    fi
    cfg="$WORK/$INSTANCE-$vt.yaml"
    bstep "render-$vt" 30 bash "$SELF" _render "$vt" "$mt" "$cfg" || { attempt "vm_start_$vt" "render failed"; continue; }
    bstep "limactl-validate-$vt" 60 limactl validate "$cfg" || { attempt "vm_start_$vt" "limactl validate failed"; continue; }
    rem=$(( DEADLINE - $(date +%s) - POST_START_SECONDS - FINAL_RESERVE ))
    if [ "$i" -lt "$ncand" ] && [ "$rem" -gt "$VZ_START_MAX" ]; then rem=$VZ_START_MAX; fi
    if [ "$rem" -lt "$MIN_START" ]; then
      attempt "vm_start_$vt" "skipped: only ${rem}s of start budget left"; continue
    fi
    if step "limactl-start-$vt" "$rem" limactl start --tty=false --name "$INSTANCE" --timeout "$(( rem / 60 ))m" "$cfg"; then
      started=1; VMTYPE=$vt; attempt "vm_start_$vt" "started ${vt} in $(( $(date +%s) - t_start ))s since probe start (budget ${rem}s)"; break
    fi
    attempt "vm_start_$vt" "FAILED: limactl start rc=$(tail -1 "$STEPS" | sed 's/.*"rc":\([0-9]*\).*/\1/') within ${rem}s (see $(basename "$LAST_LOG") and instance-$vt-failed/)"
    bstep "collect-failed-$vt" 60 bash "$SELF" _collect_instance_logs "$LOG_DIR/instance-$vt-failed"; diag "collect-failed-$vt" $?
    if ! bstep "delete-failed-$vt" 120 limactl delete -f "$INSTANCE"; then
      attempt "vm_delete_$vt" "FAILED; no further vmType attempted under the same instance name"; break
    fi
  done
  if [ "$started" != 1 ]; then
    check local_linux_vm_running 0 "no vmType started: $vmtypes"
    return 1
  fi
  check local_linux_vm_running 1 "vmType=$VMTYPE mountType=$mt"
  echo "$VMTYPE" > "$LOG_DIR/vmtype.txt"

  bstep limactl-list 60 limactl list --json; diag limactl-list $?
  bstep vm-identity 300 bash "$SELF" _vm_identity; rc=$?
  vm_host=$(sed -n 's/^hostname=//p' "$LAST_LOG" | head -1)
  p=0; [ -n "$vm_host" ] && p=1
  check vm_identity_recorded "$(both $rc $p)" "rc=$rc hostname=$vm_host"

  bstep docker-context-create 30 docker context create g7lima --description "g7plat: Docker Engine in local Lima VM $INSTANCE ($VMTYPE)" --docker "host=unix://$LIMA_HOME/$INSTANCE/sock/docker.sock"; rc=$?
  bstep docker-context-use 30 docker context use g7lima; rc2=$?
  check docker_context_configured "$(both $rc "$(b $rc2)")" "create_rc=$rc use_rc=$rc2"
  bstep docker-version 120 docker version; diag docker-version $?
  bstep docker-version-fields 120 docker version --format '{{.Client.Os}}/{{.Client.Arch}} {{.Client.Version}} | {{.Server.Os}}/{{.Server.Arch}} {{.Server.Version}} | {{.Server.Platform.Name}}'; rc=$?
  p=0; last_out | grep -q -x -F "$EXPECT_CLIENT | $EXPECT_SERVER | Docker Engine - Community" && p=1
  check client_darwin_server_linux "$(both $rc $p)" "rc=$rc $(last_out | tr '\n' ' ')"
  bstep docker-info 120 docker info; diag docker-info $?
  bstep docker-info-fields 120 docker info --format 'Name={{.Name}} OSType={{.OSType}} OperatingSystem={{.OperatingSystem}} KernelVersion={{.KernelVersion}} Architecture={{.Architecture}} NCPU={{.NCPU}}'; rc=$?
  p=0; [ -n "$vm_host" ] && last_out | grep -q "^Name=$vm_host OSType=linux " && p=1
  check daemon_is_the_local_lima_vm "$(both $rc $p)" "rc=$rc vm_hostname=$vm_host daemon: $(last_out | tr '\n' ' ')"
  bstep execution-location 60 bash "$SELF" _location; rc=$?
  p=0; grep -q "unix://$LIMA_HOME/$INSTANCE/sock/docker.sock" "$LAST_LOG" && grep -q -E 'limactl hostagent' "$LAST_LOG" && p=1
  check endpoint_is_local_forwarded_socket "$(both $rc $p)" "rc=$rc; unix socket under LIMA_HOME, served by this host's Lima hostagent"

  marker="G7PLAT-MACOS-$RUN_TAG-$VMTYPE"
  bstep container-smoke 600 docker run --rm --name "g7plat-smoke-$RUN_TAG" "$SMOKE_IMAGE" sh -c "echo $marker; echo container_hostname=\$(hostname); uname -srm; head -n 2 /etc/os-release; id"; rc=$?
  p=0; last_out | grep -q -x "$marker" && p=1
  check disposable_container_output "$(both $rc $p)" "rc=$rc; marker $marker in container stdout"

  printf 'host-%s\n' "$RUN_TAG" > "$BIND_DIR/from-host.txt"
  bstep bind-same-path-rw 300 docker run --rm --name "g7plat-bind-$RUN_TAG" -v "$BIND_DIR:$BIND_DIR" "$SMOKE_IMAGE" sh -c "cat '$BIND_DIR/from-host.txt' && echo ctr-$RUN_TAG > '$BIND_DIR/from-container.txt' && ls -ln '$BIND_DIR'"; rc=$?
  reads=$(last_out | grep -c -x "host-$RUN_TAG")
  back=$(cat "$BIND_DIR/from-container.txt" 2>/dev/null || true)
  p=0; [ "$reads" -ge 1 ] && p=1
  check bind_same_path_read "$(both $rc $p)" "rc=$rc; container read host file at $BIND_DIR"
  p=0; [ "$back" = "ctr-$RUN_TAG" ] && p=1
  check bind_same_path_write "$(both $rc $p)" "rc=$rc; host read back [$back] (owner/mode $(stat -f '%u:%g %Sp' "$BIND_DIR/from-container.txt" 2>/dev/null || echo n/a))"

  bstep leftover-containers 60 docker ps -a --filter name=g7plat- --format '{{.ID}} {{.Names}} {{.Status}}'; rc=$?
  p=0; [ -z "$(last_out)" ] && p=1
  check no_leftover_containers "$(both $rc $p)" "rc=$rc $(last_out | tr '\n' ' ')"
  bstep guest-cloud-init-log 120 limactl shell --workdir / "$INSTANCE" sudo tail -n 400 /var/log/cloud-init-output.log; diag guest-cloud-init-log $?
  bstep collect-instance-logs 60 bash "$SELF" _collect_instance_logs "$LOG_DIR/instance-$VMTYPE"; diag collect-instance-logs $?
  return 0
}

# ------------------------------------------------------------- cleanup ------
# Bounded by its own shared deadline inside the 4-minute workflow step. The
# VM's control directory is deleted only after the instance is confirmed
# stopped; otherwise it is preserved (with its logs) for disposal with the
# hosted runner, and the cleanup reports INCOMPLETE.
cmd_cleanup() {
  local fail=0 stopped=0 reasons="" d rc
  DEADLINE=$(( $(date +%s) + CLEANUP_BUDGET_SECONDS )); FINAL_RESERVE=5
  # shellcheck source=/dev/null  # written by probe in this job: printf %q-quoted paths only
  [ -f "$LOG_DIR/state.env" ] && . "$LOG_DIR/state.env"
  d="$LIMA_HOME/$INSTANCE"
  if [ -d "$d" ] && command -v limactl >/dev/null 2>&1; then
    if [ ! -d "$LOG_DIR/instance-final" ]; then
      bstep cleanup-collect-logs 20 bash "$SELF" _collect_instance_logs "$LOG_DIR/instance-final"; diag cleanup-collect-logs $?
    fi
    if [ -S "$d/sock/docker.sock" ]; then
      # shellcheck disable=SC2016  # expanded by the inner bash
      bstep cleanup-containers 30 bash -c 'ids=$(docker ps -aq --filter name=g7plat-); [ -z "$ids" ] || docker rm -f $ids'
      diag cleanup-containers $?
    fi
    bstep cleanup-vm-stop 90 limactl stop -f "$INSTANCE"; diag cleanup-vm-stop $?
    if bstep cleanup-vm-stopped 20 bash "$SELF" _instance_stopped; then
      stopped=1
      bstep cleanup-vm-delete 40 limactl delete -f "$INSTANCE"; rc=$?
      if [ "$rc" != 0 ] || [ -e "$d" ]; then fail=1; reasons="$reasons vm-delete(rc=$rc)"; fi
    else
      fail=1; reasons="$reasons vm-not-confirmed-stopped(control directory and logs preserved for runner disposal)"
    fi
  elif [ -d "$d" ]; then
    fail=1; reasons="$reasons limactl-unavailable(instance directory preserved)"
  fi
  # The bind dir, client config and downloads never control the VM; LIMA_HOME
  # and the tools are removed only once the instance is gone or confirmed stopped.
  set -- "$BIND_DIR" "$DOCKER_CONFIG" "$WORK/dl"
  if [ "$stopped" = 1 ] || [ ! -d "$d" ]; then set -- "$@" "$LIMA_HOME" "$WORK/opt"; fi
  bstep cleanup-files 30 bash "$SELF" _safe_rm "$@"; rc=$?
  [ "$rc" = 0 ] || { fail=1; reasons="$reasons owned-path-removal(rc=$rc)"; }
  bstep cleanup-verify 15 bash "$SELF" _verify_gone "$@"; rc=$?
  [ "$rc" = 0 ] || { fail=1; reasons="$reasons verification(rc=$rc)"; }
  if [ "$fail" = 0 ]; then
    echo "cleanup=COMPLETE" | tee "$LOG_DIR/CLEANUP.txt"; return 0
  fi
  echo "cleanup=INCOMPLETE reasons:$reasons" | tee "$LOG_DIR/CLEANUP.txt"; return 1
}

summarize() {
  local verdict=$1
  {
    echo "verdict=$verdict failed_checks=$FAILED"
    echo "vmtype=$(cat "$LOG_DIR/vmtype.txt" 2>/dev/null || echo none)"
    echo "--- checks (PASS/FAIL decide; ATTEMPT and DIAG are evidence only)"; cat "$CHECKS" 2>/dev/null
    echo "--- not claimed: Gate7 acceptance, product behaviour, native macOS container execution (containers run in the local Linux VM)"
  } | tee "$LOG_DIR/SUMMARY.txt"
}

case ${1:-} in
  probe)
    cmd_probe; rc=$?
    if [ "$rc" = 0 ] && [ "$FAILED" = 0 ]; then summarize CAPABILITY_OBSERVED; exit 0; fi
    summarize CAPABILITY_NOT_ESTABLISHED; exit 1 ;;
  cleanup) cmd_cleanup ;;
  _fetch) shift; cmd_fetch "$@" ;;
  _host_identity) cmd_host_identity ;;
  _qemu_identity) cmd_qemu_identity ;;
  _render) shift; cmd_render "$@" ;;
  _collect_instance_logs) shift; cmd_collect_instance_logs "$@" ;;
  _vm_identity) cmd_vm_identity ;;
  _location) cmd_location ;;
  _instance_stopped) cmd_instance_stopped ;;
  _safe_rm) shift; cmd_safe_rm "$@" ;;
  _verify_gone) shift; cmd_verify_gone "$@" ;;
  *) sed -n '2,21p' "$0"; exit 2 ;;
esac
