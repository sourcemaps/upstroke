#!/usr/bin/env bash
# macos-docker-capability.sh -- bounded macOS Docker capability probe
# (G7-PLATFORM-ENVIRONMENT-SETUP-20261108-1).
#
# Ordinary environment-capability evidence on a standard public GitHub-hosted
# macOS Intel runner: the native macOS host drives a pinned Docker CLI against
# Docker Engine inside a task-owned local Linux VM (Lima). This is NOT Gate7
# acceptance, not a required check, and asserts nothing about the product.
#
#   probe    host identity -> pinned downloads -> local VM (vz when
#            Hypervisor.framework is usable, else QEMU; Lima >= 2.2 falls back
#            from HVF to TCG software emulation) -> Docker client and daemon
#            identity -> execution location -> one disposable container ->
#            same-path read/write bind mount
#   cleanup  bounded teardown of everything probe created; safe to re-run
#
# Every step's output and exit status is kept under $G7_LOG_DIR. Nothing is
# retried silently and no failure is discarded. No secret is read; only an
# allowlist of runner variables is recorded. Written for macOS /bin/bash 3.2.
set -uo pipefail

# ---------------------------------------------------------------- pins ------
# Lima v2.2.1 is the newest release older than four weeks that contains
# lima-vm/lima#5137 (merged 2026-06-22: QEMU falls back from HVF to TCG).
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
# The probe step gets 38 minutes from the workflow; keep our own budget inside it.
BUDGET_SECONDS=${G7_BUDGET_SECONDS:-2160}
RESERVE_SECONDS=420   # kept back for identity, container, bind mount and log capture
STEPS="$LOG_DIR/steps.jsonl"
CHECKS="$LOG_DIR/checks.tsv"
mkdir -p "$LOG_DIR" "$WORK"

# -------------------------------------------------------------- helpers -----
now() { date -u +%Y-%m-%dT%H:%M:%SZ; }
SEQ=0
[ -f "$STEPS" ] && SEQ=$(wc -l < "$STEPS" | tr -d ' ')
LAST_LOG=/dev/null
FAILED=0

# Run "$@" with a hard wall-clock limit; exit 124 on timeout (as GNU timeout).
# A supervisor, not alarm+exec: Go programs (limactl, docker) ignore SIGALRM.
# The child gets its own process group, so a timeout stops its whole subtree.
# shellcheck disable=SC2016  # a Perl program, not shell
BOUNDED_PL='
use strict; use warnings; use POSIX ":sys_wait_h";
my $t = shift @ARGV; my $pid = fork();
defined $pid or die "fork failed: $!\n";
if ($pid == 0) { setpgrp(0, 0); exec { $ARGV[0] } @ARGV or do { print STDERR "exec failed: $!\n"; POSIX::_exit(127) }; }
my $deadline = time + $t;
while (1) {
  if (waitpid($pid, WNOHANG) == $pid) { my $s = $?; exit(($s & 127) ? 128 + ($s & 127) : $s >> 8); }
  if (time >= $deadline) {
    print STDERR "bounded: limit ${t}s reached, sending TERM to process group $pid\n";
    kill "-TERM", $pid; my $g = time + 5;
    while (time < $g) { last if waitpid($pid, WNOHANG) == $pid; select(undef, undef, undef, 0.2); }
    kill "-KILL", $pid; waitpid($pid, 0); exit 124;
  }
  select(undef, undef, undef, 0.2);
}'
bounded() {
  local secs=$1 pid watcher rc
  shift
  if command -v perl >/dev/null 2>&1; then
    perl -e "$BOUNDED_PL" "$secs" "$@"
    return $?
  fi
  "$@" & pid=$!
  ( sleep "$secs"; kill -TERM "$pid" 2>/dev/null; sleep 5; kill -KILL "$pid" 2>/dev/null ) & watcher=$!
  wait "$pid"; rc=$?
  pkill -P "$watcher" 2>/dev/null; kill "$watcher" 2>/dev/null; wait "$watcher" 2>/dev/null
  return "$rc"
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
  case $rc in 124|137|143) printf '# TIMEOUT or kill (rc=%s) after limit %ss\n' "$rc" "$secs" >> "$f" ;; esac
  printf '# %s exit=%s secs=%s\n' "$(now)" "$rc" "$(( $(date +%s) - t0 ))" >> "$f"
  printf '{"seq":%d,"step":"%s","rc":%d,"secs":%d,"log":"%s"}\n' \
    "$SEQ" "$name" "$rc" "$(( $(date +%s) - t0 ))" "$(basename "$f")" >> "$STEPS"
  LAST_LOG=$f
  return "$rc"
}

# Output of the last step without its '# ' framing lines.
last_out() { grep -v '^# ' "$LAST_LOG"; }

# b RC -> 1 when RC is 0, else 0 (for check()).
b() { if [ "$1" = 0 ]; then echo 1; else echo 0; fi; }

# check NAME 0|1 DETAIL
check() {
  local verdict=FAIL
  if [ "$2" = 1 ]; then verdict=PASS; else FAILED=$((FAILED + 1)); fi
  printf '%s\t%s\t%s\n' "$verdict" "$1" "$(printf '%s' "$3" | tr '\t\n' '  ' | cut -c1-300)" | tee -a "$CHECKS"
}

# attempt NAME DETAIL -- an outcome kept as evidence that does not decide the
# verdict (e.g. vz failing before a QEMU fallback succeeds).
attempt() {
  printf 'ATTEMPT\t%s\t%s\n' "$1" "$(printf '%s' "$2" | tr '\t\n' '  ' | cut -c1-300)" | tee -a "$CHECKS"
}

sha256_of() {
  if command -v shasum >/dev/null 2>&1; then shasum -a 256 "$1" | awk '{print $1}'
  else openssl dgst -sha256 -r "$1" | awk '{print $1}'; fi
}

remaining() { echo $(( DEADLINE - $(date +%s) )); }

# --------------------------------------------------- internal subcommands ---
# fetch URL SHA256 DEST: download, then refuse anything but the pinned digest.
cmd_fetch() {
  local url=$1 want=$2 dest=$3 got
  curl -fsSL --retry 3 --retry-delay 5 --max-time 600 -o "$dest" "$url" || return 1
  got=$(sha256_of "$dest")
  echo "url=$url"; echo "sha256=$got"; echo "expected=$want"; echo "bytes=$(wc -c < "$dest" | tr -d ' ')"
  [ "$got" = "$want" ] || { echo "DIGEST MISMATCH: refusing $dest"; rm -f "$dest"; return 1; }
  echo "digest OK"
}

cmd_host_identity() {
  local k v
  echo "date_utc=$(now)"
  sw_vers
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
  local sock="$LIMA_HOME/$INSTANCE/sock/docker.sock"
  echo "--- client endpoint"; docker context inspect g7lima --format '{{json .Endpoints.docker}}'
  echo "--- forwarded socket on this host"; ls -l "$sock"
  echo "--- VM processes on this host (Lima hostagent and hypervisor)"
  # shellcheck disable=SC2009  # full command lines with ppid/etime are the evidence wanted here
  ps -axo pid,ppid,etime,command | grep -E '[l]imactl hostagent|[q]emu-system|[V]irtualization\.VirtualMachine' | cut -c1-400
  echo "--- accelerator evidence (QEMU only; vz uses Virtualization.framework)"
  # shellcheck disable=SC2009  # the QEMU argv itself is the evidence
  ps -axo command | grep -E '[q]emu-system' | tr ' ' '\n' | grep -A1 -E '^-accel$|^-machine$' | tr '\n' ' '; echo
  grep -h -i -E 'hvf|tcg|accel' "$LIMA_HOME/$INSTANCE"/*.log 2>/dev/null | tail -20 || true
}

# --------------------------------------------------------------- probe ------
cmd_probe() {
  local t_start vmtypes vt mt started=0 cfg rem marker created back rc
  t_start=$(date +%s); DEADLINE=$(( t_start + BUDGET_SECONDS ))
  : > "$CHECKS"
  printf 'RUN_TAG=%q\nLIMA_HOME=%q\nBIND_DIR=%q\nWORK=%q\nINSTANCE=%q\n' "$RUN_TAG" "$LIMA_HOME" "$BIND_DIR" "$WORK" "$INSTANCE" > "$LOG_DIR/state.env"

  step host-identity 120 bash "$SELF" _host_identity; rc=$?
  check host_identity_recorded "$(b $rc)" "$(grep -E '^(ProductVersion|kern.hv_support|kern.hv_vmm_present|hw.model)' "$LAST_LOG" | tr '\n' ' ')"

  mkdir -p "$WORK/dl" "$OPT/lima"
  step fetch-lima 660 bash "$SELF" _fetch "$LIMA_URL" "$LIMA_SHA256" "$WORK/dl/lima.tar.gz"; rc=$?
  check lima_download_pinned "$(b $rc)" "lima ${LIMA_VERSION} sha256 ${LIMA_SHA256}"
  step fetch-docker-cli 660 bash "$SELF" _fetch "$DOCKER_CLI_URL" "$DOCKER_CLI_SHA256" "$WORK/dl/docker.tgz"; rc=$?
  check docker_cli_download_pinned "$(b $rc)" "docker ${DOCKER_CLI_VERSION} sha256 ${DOCKER_CLI_SHA256}"
  if [ ! -s "$WORK/dl/lima.tar.gz" ] || [ ! -s "$WORK/dl/docker.tgz" ]; then
    check pinned_tools_available 0 "a pinned download failed; see fetch logs"; return 1
  fi
  # shellcheck disable=SC2016  # positional parameters of the inner bash, passed after "_"
  step unpack-tools 120 bash -c 'tar -xzf "$1" -C "$2/lima" && tar -xzf "$3" -C "$2" && ls -l "$2/lima/bin" "$2/docker"' _ "$WORK/dl/lima.tar.gz" "$OPT" "$WORK/dl/docker.tgz"
  # shellcheck disable=SC2016  # expanded by the inner bash, which inherits PATH
  step tool-identity 60 bash -c 'limactl --version; docker --version; file "$(command -v limactl)" "$(command -v docker)"; codesign -dv --entitlements - "$(command -v limactl)" 2>&1 | grep -E "Identifier|Signature|virtualization|hypervisor" || true'

  # vz needs Hypervisor.framework; QEMU is tried after it, or alone without it.
  case ${G7_VMTYPES:-auto} in
    auto) if [ "$(sysctl -n kern.hv_support 2>/dev/null || echo 0)" = 1 ]; then vmtypes="vz qemu"; else vmtypes="qemu"; fi ;;
    *) vmtypes=$G7_VMTYPES ;;
  esac
  echo "vmType order: $vmtypes (kern.hv_support=$(sysctl -n kern.hv_support 2>/dev/null || echo n/a))" | tee "$LOG_DIR/vmtype-order.txt"
  mkdir -p "$BIND_DIR"
  for vt in $vmtypes; do
    if [ "$vt" = vz ]; then mt=virtiofs; else mt=9p; fi
    if [ "$vt" = qemu ] && ! command -v qemu-system-x86_64 >/dev/null 2>&1; then
      # QEMU comes from the runner image's Homebrew snapshot (auto-update off):
      # not digest-pinned by this workflow; its version and bottle are recorded.
      step brew-install-qemu 900 brew install qemu
      # shellcheck disable=SC2016  # jq program, not shell
      step qemu-identity 60 bash -c 'qemu-system-x86_64 --version; brew info --json=v2 --installed qemu | jq -c ".formulae[0] | {name, installed: [.installed[] | {version, poured_from_bottle, built_as_bottle}], bottle_root_url: .bottle.stable.root_url}"'
    fi
    cfg="$WORK/$INSTANCE-$vt.yaml"
    step "render-$vt" 30 bash "$SELF" _render "$vt" "$mt" "$cfg" || continue
    step "limactl-validate-$vt" 60 limactl validate "$cfg"
    rem=$(( $(remaining) - RESERVE_SECONDS ))
    if [ "$rem" -lt 300 ]; then
      attempt "vm_start_$vt" "skipped: only ${rem}s of budget left"; continue
    fi
    if step "limactl-start-$vt" "$rem" limactl start --tty=false --name "$INSTANCE" --timeout "$(( rem / 60 ))m" "$cfg"; then
      started=1; VMTYPE=$vt; attempt "vm_start_$vt" "started ${vt} in $(( $(date +%s) - t_start ))s since probe start"; break
    fi
    attempt "vm_start_$vt" "FAILED: limactl start rc=$(tail -1 "$STEPS" | sed 's/.*"rc":\([0-9]*\).*/\1/') (see $(basename "$LAST_LOG") and instance-$vt-failed/)"
    step "collect-failed-$vt" 60 bash "$SELF" _collect_instance_logs "$LOG_DIR/instance-$vt-failed"
    step "delete-failed-$vt" 180 limactl delete -f "$INSTANCE"
  done
  if [ "$started" != 1 ]; then
    check local_linux_vm_running 0 "no vmType started: $vmtypes"
    return 1
  fi
  check local_linux_vm_running 1 "vmType=$VMTYPE mountType=$mt"
  echo "$VMTYPE" > "$LOG_DIR/vmtype.txt"

  step limactl-list 60 limactl list --json
  step vm-identity 300 bash "$SELF" _vm_identity
  local vm_host; vm_host=$(sed -n 's/^hostname=//p' "$LAST_LOG" | head -1)

  step docker-context-create 30 docker context create g7lima --description "g7plat: Docker Engine in local Lima VM $INSTANCE ($VMTYPE)" --docker "host=unix://$LIMA_HOME/$INSTANCE/sock/docker.sock"
  step docker-context-use 30 docker context use g7lima
  step docker-version 120 docker version
  step docker-version-fields 120 docker version --format '{{.Client.Os}}/{{.Client.Arch}} {{.Client.Version}} | {{.Server.Os}}/{{.Server.Arch}} {{.Server.Version}} | {{.Server.Platform.Name}}'
  check client_darwin_server_linux "$(last_out | grep -q -x -F "$EXPECT_CLIENT | $EXPECT_SERVER | Docker Engine - Community" && echo 1 || echo 0)" "$(last_out | tr '\n' ' ')"
  step docker-info 120 docker info
  step docker-info-fields 120 docker info --format 'Name={{.Name}} OSType={{.OSType}} OperatingSystem={{.OperatingSystem}} KernelVersion={{.KernelVersion}} Architecture={{.Architecture}} NCPU={{.NCPU}}'
  check daemon_is_the_local_lima_vm "$( [ -n "$vm_host" ] && last_out | grep -q "^Name=$vm_host OSType=linux " && echo 1 || echo 0)" "vm_hostname=$vm_host daemon: $(last_out | tr '\n' ' ')"
  step execution-location 60 bash "$SELF" _location
  check endpoint_is_local_forwarded_socket "$(grep -q "unix://$LIMA_HOME/$INSTANCE/sock/docker.sock" "$LAST_LOG" && grep -q -E 'limactl hostagent' "$LAST_LOG" && echo 1 || echo 0)" "unix socket under LIMA_HOME, served by this host's Lima hostagent"

  marker="G7PLAT-MACOS-$RUN_TAG-$VMTYPE"
  step container-smoke 600 docker run --rm --name "g7plat-smoke-$RUN_TAG" "$SMOKE_IMAGE" sh -c "echo $marker; echo container_hostname=\$(hostname); uname -srm; head -n 2 /etc/os-release; id"
  check disposable_container_output "$(last_out | grep -q -x "$marker" && echo 1 || echo 0)" "marker $marker in container stdout"

  printf 'host-%s\n' "$RUN_TAG" > "$BIND_DIR/from-host.txt"
  step bind-same-path-rw 300 docker run --rm --name "g7plat-bind-$RUN_TAG" -v "$BIND_DIR:$BIND_DIR" "$SMOKE_IMAGE" sh -c "cat '$BIND_DIR/from-host.txt' && echo ctr-$RUN_TAG > '$BIND_DIR/from-container.txt' && ls -ln '$BIND_DIR'"
  created=$(last_out | grep -c -x "host-$RUN_TAG")
  back=$(cat "$BIND_DIR/from-container.txt" 2>/dev/null || true)
  check bind_same_path_read "$([ "$created" -ge 1 ] && echo 1 || echo 0)" "container read host file at $BIND_DIR"
  check bind_same_path_write "$([ "$back" = "ctr-$RUN_TAG" ] && echo 1 || echo 0)" "host read back [$back] (owner/mode $(stat -f '%u:%g %Sp' "$BIND_DIR/from-container.txt" 2>/dev/null || echo n/a))"

  step leftover-containers 60 docker ps -a --filter name=g7plat- --format '{{.ID}} {{.Names}} {{.Status}}'
  check no_leftover_containers "$([ -z "$(last_out)" ] && echo 1 || echo 0)" "$(last_out | tr '\n' ' ')"
  step guest-cloud-init-log 120 limactl shell --workdir / "$INSTANCE" sudo tail -n 400 /var/log/cloud-init-output.log
  step collect-instance-logs 60 bash "$SELF" _collect_instance_logs "$LOG_DIR/instance-$VMTYPE"
  return 0
}

# ------------------------------------------------------------- cleanup ------
cmd_cleanup() {
  # shellcheck source=/dev/null  # written by probe in this job: printf %q-quoted paths only
  [ -f "$LOG_DIR/state.env" ] && . "$LOG_DIR/state.env"
  if command -v docker >/dev/null 2>&1 && [ -S "$LIMA_HOME/$INSTANCE/sock/docker.sock" ]; then
    # shellcheck disable=SC2016  # expanded by the inner bash
    step cleanup-containers 60 bash -c 'ids=$(docker ps -aq --filter name=g7plat-); [ -z "$ids" ] || docker rm -f $ids'
  fi
  if command -v limactl >/dev/null 2>&1 && [ -d "$LIMA_HOME/$INSTANCE" ]; then
    [ -d "$LOG_DIR/instance-final" ] || step cleanup-collect-logs 60 bash "$SELF" _collect_instance_logs "$LOG_DIR/instance-final"
    step cleanup-vm-stop 150 limactl stop -f "$INSTANCE"
    step cleanup-vm-delete 150 limactl delete -f "$INSTANCE"
  fi
  step cleanup-files 120 bash "$SELF" _safe_rm "$BIND_DIR" "$LIMA_HOME" "$DOCKER_CONFIG" "$WORK/opt" "$WORK/dl"
  # shellcheck disable=SC2016  # positional parameters of the inner bash, passed after "_"
  step cleanup-verify 30 bash -c 'echo "--- remaining VM processes (expect none)"; ps -axo pid,command | grep -E "[l]imactl hostagent|[q]emu-system" || echo none; for p in "$@"; do [ -e "$p" ] && echo "still present: $p" || echo "removed: $p"; done' _ "$BIND_DIR" "$LIMA_HOME"
  return 0
}

# Remove only this probe's own paths; refuse anything else.
cmd_safe_rm() {
  local p
  for p in "$@"; do
    case $p in
      "$HOME"/.g7lima|"$HOME"/g7plat-bind-*|"$WORK"/opt|"$WORK"/dl|"$WORK"/docker-config) ;;
      *)
        if [ -z "${G7_LIMA_HOME:-}" ] || [ "$p" != "$G7_LIMA_HOME" ]; then
          echo "refusing to remove unexpected path: $p"; continue
        fi ;;
    esac
    case $p in ""|/|"$HOME"|"$HOME"/) echo "refusing: $p"; continue ;; esac
    rm -rf -- "$p" && echo "removed $p"
  done
  return 0
}

summarize() {
  local verdict=$1
  {
    echo "verdict=$verdict failed_checks=$FAILED"
    echo "vmtype=$(cat "$LOG_DIR/vmtype.txt" 2>/dev/null || echo none)"
    echo "--- checks"; cat "$CHECKS" 2>/dev/null
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
  _render) shift; cmd_render "$@" ;;
  _collect_instance_logs) shift; cmd_collect_instance_logs "$@" ;;
  _vm_identity) cmd_vm_identity ;;
  _location) cmd_location ;;
  _safe_rm) shift; cmd_safe_rm "$@" ;;
  *) sed -n '2,19p' "$0"; exit 2 ;;
esac
