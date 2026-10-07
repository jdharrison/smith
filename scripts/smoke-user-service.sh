#!/usr/bin/env bash
set -euo pipefail

readonly WORK_RECORD="LOCAL-SMITH-CLI-LIFECYCLE-HARDENING-P0"
readonly PLAN_BASE_REVISION="825b8afe7aed14a44efda0c58fc74e65b8fb02b6"

usage() {
    cat <<'EOF'
Usage: scripts/smoke-user-service.sh --sandbox

Exercise Smith's Ubuntu user-service CLI only against temporary state and fake
systemctl/loginctl programs. There is intentionally no live-manager mode.

Each invocation creates a fresh, checksummed, owner-read-only evidence directory.
The checksum package detects stale mixing and accidental changes; release evidence
must anchor its digest in an independent review or approval record.

Optional environment:
  SMITH_SMOKE_BINARY       Existing Smith binary to copy instead of running the installer.
  SMITH_SMOKE_ARTIFACT_DIR Evidence-runs parent (default: target/smoke-user-service/runs).
  SMITH_SMOKE_TMPDIR       Stable parent directory for the temporary sandbox.
                           Default: $HOME/.local/share/smith/service-smoke.
                           It must not be under /tmp, /var/tmp, /dev, or /run.
EOF
}

fail() {
    printf 'smoke-user-service: %s\n' "$*" >&2
    overall_result="fail"
    exit 1
}

block() {
    printf 'smoke-user-service: BLOCKED: %s\n' "$*" >&2
    overall_result="blocked"
    exit 2
}

if (($# != 1)) || [[ "$1" != "--sandbox" ]]; then
    usage >&2
    fail "--sandbox is required; live user-manager execution is not supported"
fi

for required in bash date git mktemp python3 readlink sha256sum stat uname; do
    command -v "$required" >/dev/null 2>&1 || block "$required is required"
done

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
repo_root="$(cd -- "$script_dir/.." && pwd -P)"
original_home="${HOME:?HOME is required}"
export CARGO_HOME="${CARGO_HOME:-$original_home/.cargo}"
export RUSTUP_HOME="${RUSTUP_HOME:-$original_home/.rustup}"
artifact_root="${SMITH_SMOKE_ARTIFACT_DIR:-$repo_root/target/smoke-user-service/runs}"
mkdir -p -- "$artifact_root"
artifact_root="$(cd -- "$artifact_root" && pwd -P)"

started_at_utc="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
started_at_compact="$(date -u +%Y%m%dT%H%M%SZ)"
branch="$(git -C "$repo_root" branch --show-current)"
[[ -n "$branch" ]] || branch="DETACHED"
head_revision="$(git -C "$repo_root" rev-parse HEAD)"
head_tree="$(git -C "$repo_root" rev-parse 'HEAD^{tree}')"
head_short="$(git -C "$repo_root" rev-parse --short=12 HEAD)"
run_dir="$(mktemp -d "$artifact_root/${started_at_compact}-${head_short}.XXXXXX")"
chmod 700 "$run_dir"
mkdir -p -- "$run_dir/commands" "$run_dir/repository" "$run_dir/systemd-verify"
transcript="$run_dir/transcript.log"
manager_log="$run_dir/manager.log"
command_results="$run_dir/commands/results.tsv"
: >"$transcript"
: >"$manager_log"
printf 'sequence\tlabel\texit_status\texpectation\n' >"$command_results"

capture_source_inputs() {
    local destination="$1" path
    : >"$destination"
    while IFS= read -r -d '' path; do
        if [[ -f "$repo_root/$path" ]]; then
            sha256sum --zero -- "$path" >>"$destination"
        fi
    done < <(git ls-files -z --cached --others --exclude-standard)
}

main_smoke() {
(
    cd -- "$repo_root"
    git status --porcelain=v2 --branch >"$run_dir/repository/status.before.txt"
    git diff --binary --full-index HEAD -- >"$run_dir/repository/dirty.before.patch"
    capture_source_inputs "$run_dir/repository/source-inputs.before.sha256z"
)

dirty_diff_sha256="$(sha256sum "$run_dir/repository/dirty.before.patch" | awk '{print $1}')"
source_inputs_before_sha256="$(sha256sum "$run_dir/repository/source-inputs.before.sha256z" | awk '{print $1}')"
if [[ -s "$run_dir/repository/dirty.before.patch" ]] || grep -q '^? ' "$run_dir/repository/status.before.txt"; then
    worktree_dirty="yes"
else
    worktree_dirty="no"
fi

if [[ "$branch" != "feature/autonomy" || "$head_revision" != "$PLAN_BASE_REVISION" ]]; then
    block "active plan binding mismatch (expected feature/autonomy at $PLAN_BASE_REVISION)"
fi

tmp_parent="${SMITH_SMOKE_TMPDIR:-$original_home/.local/share/smith/service-smoke}"
mkdir -p -- "$tmp_parent"
tmp_parent="$(cd -- "$tmp_parent" && pwd -P)"
case "$tmp_parent" in
    /tmp|/tmp/*|/var/tmp|/var/tmp/*|/dev|/dev/*|/run|/run/*)
        fail "SMITH_SMOKE_TMPDIR must allow a temporary but service-stable executable path: $tmp_parent"
        ;;
esac
smoke_root="$(mktemp -d "$tmp_parent/run.XXXXXX")"
case "$run_dir" in
    "$smoke_root"|"$smoke_root"/*) fail "evidence directory must be outside the disposable sandbox" ;;
esac

home_root="$smoke_root/home"
xdg_config_root="$smoke_root/xdg/config"
xdg_data_root="$smoke_root/xdg/data"
xdg_cache_root="$smoke_root/xdg/cache"
xdg_runtime_root="$smoke_root/xdg/runtime"
install_root="$smoke_root/install"
fake_bin="$smoke_root/fake-bin"
manager_state="$smoke_root/manager-state"
unmanaged_log="$smoke_root/unmanaged.log"
mkdir -p -- \
    "$home_root" \
    "$xdg_config_root" \
    "$xdg_data_root" \
    "$xdg_cache_root" \
    "$xdg_runtime_root" \
    "$install_root/bin" \
    "$fake_bin" \
    "$manager_state"
chmod 700 "$xdg_runtime_root" "$install_root"
: >"$unmanaged_log"
printf 'inactive\n' >"$manager_state/active"
printf 'dead\n' >"$manager_state/substate"
printf 'disabled\n' >"$manager_state/enabled"
printf 'success\n' >"$manager_state/result"
printf '0\n' >"$manager_state/restarts"
printf '1\n' >"$manager_state/main-code"
printf '0\n' >"$manager_state/main-status"
printf 'no\n' >"$manager_state/malformed"
printf 'no\n' >"$manager_state/multiline-enablement"
printf 'no\n' >"$manager_state/multiline-linger"

export HOME="$home_root"
export USER="smith-smoke"
export LOGNAME="$USER"
export XDG_CONFIG_HOME="$xdg_config_root"
export XDG_DATA_HOME="$xdg_data_root"
export XDG_CACHE_HOME="$xdg_cache_root"
export XDG_RUNTIME_DIR="$xdg_runtime_root"
export DBUS_SESSION_BUS_ADDRESS="unix:path=$xdg_runtime_root/no-live-user-bus"
export SMITH_SMOKE_MANAGER_STATE="$manager_state"
export SMITH_SMOKE_MANAGER_LOG="$manager_log"
export SMITH_TEST_SUPPRESS_EXTERNAL_STATUS_PROBES=1
smith_bin="$install_root/bin/smith"

cat >"$fake_bin/systemctl" <<'FAKE_SYSTEMCTL_SENTINEL'
#!/usr/bin/env bash
set -euo pipefail
entry="EARLY-SYSTEMCTL"
for arg in "$@"; do
    printf -v quoted '%q' "$arg"
    entry+=" $quoted"
done
printf '%s\n' "$entry" >>"${SMITH_SMOKE_MANAGER_LOG:?}"
printf 'fake systemctl: manager access is forbidden during smoke bootstrap\n' >&2
exit 99
FAKE_SYSTEMCTL_SENTINEL
cat >"$fake_bin/loginctl" <<'FAKE_LOGINCTL_SENTINEL'
#!/usr/bin/env bash
set -euo pipefail
entry="EARLY-LOGINCTL"
for arg in "$@"; do
    printf -v quoted '%q' "$arg"
    entry+=" $quoted"
done
printf '%s\n' "$entry" >>"${SMITH_SMOKE_MANAGER_LOG:?}"
printf 'fake loginctl: manager access is forbidden during smoke bootstrap\n' >&2
exit 99
FAKE_LOGINCTL_SENTINEL
chmod 755 "$fake_bin/systemctl" "$fake_bin/loginctl"
export PATH="$fake_bin:$PATH"
hash -r
[[ "$(command -v systemctl)" == "$fake_bin/systemctl" ]] || fail "systemctl is not shadowed by the sandbox sentinel"
[[ "$(command -v loginctl)" == "$fake_bin/loginctl" ]] || fail "loginctl is not shadowed by the sandbox sentinel"

record() {
    printf '%s\n' "$*" | tee -a "$transcript"
}

command_sequence=0
CAPTURE_STATUS=0
CAPTURE_DIR=""
LAST_OUTPUT=""
LAST_MANAGER_SEGMENT=""

capture_command() {
    local label="$1" expectation="$2" before after first arg rendered=""
    shift 2
    command_sequence=$((command_sequence + 1))
    CAPTURE_DIR="$(printf '%s/commands/%03d-%s' "$run_dir" "$command_sequence" "$label")"
    mkdir -p -- "$CAPTURE_DIR"
    for arg in "$@"; do
        printf -v rendered '%s %q' "$rendered" "$arg"
    done
    printf '%s\n' "${rendered# }" >"$CAPTURE_DIR/command.txt"
    printf '%s\n' "$expectation" >"$CAPTURE_DIR/expected.txt"
    before="$(wc -l <"$manager_log")"
    if "$@" >"$CAPTURE_DIR/output.log" 2>&1; then
        CAPTURE_STATUS=0
    else
        CAPTURE_STATUS=$?
    fi
    printf '%s\n' "$CAPTURE_STATUS" >"$CAPTURE_DIR/exit-status.txt"
    after="$(wc -l <"$manager_log")"
    first=$((before + 1))
    : >"$CAPTURE_DIR/manager.log"
    if ((after >= first)); then
        sed -n "${first},${after}p" "$manager_log" >"$CAPTURE_DIR/manager.log"
    fi
    : >"$CAPTURE_DIR/unmanaged-pids.log"
    printf '%03d\t%s\t%s\t%s\n' \
        "$command_sequence" "$label" "$CAPTURE_STATUS" "$expectation" >>"$command_results"
    record "$ ${rendered# }"
    tee -a "$transcript" <"$CAPTURE_DIR/output.log" >/dev/null
    if [[ -s "$CAPTURE_DIR/manager.log" ]]; then
        tee -a "$transcript" <"$CAPTURE_DIR/manager.log" >/dev/null
    fi
    LAST_OUTPUT="$CAPTURE_DIR/output.log"
    LAST_MANAGER_SEGMENT="$CAPTURE_DIR/manager.log"
}

require_zero() {
    local label="$1"
    ((CAPTURE_STATUS == 0)) || fail "$label failed with exit status $CAPTURE_STATUS (see $CAPTURE_DIR)"
}

require_nonzero_matching() {
    local label="$1" pattern="$2"
    ((CAPTURE_STATUS != 0)) || fail "$label unexpectedly succeeded"
    grep -Eq "$pattern" "$LAST_OUTPUT" || fail "$label returned the wrong error"
}

for required in cargo shellcheck; do
    command -v "$required" >/dev/null 2>&1 || block "$required is required for the complete lifecycle evidence package"
done

capture_command rustfmt zero cargo fmt --check
require_zero "cargo fmt --check"
capture_command locked-build zero cargo build --locked
require_zero "cargo build --locked"
capture_command locked-tests zero cargo test --locked
require_zero "cargo test --locked"
capture_command strict-clippy zero cargo clippy --locked --all-targets -- -D warnings
require_zero "cargo clippy"
capture_command shell-syntax zero bash -n "$repo_root/scripts/install.sh" "$repo_root/scripts/smoke-user-service.sh"
require_zero "shell syntax"
capture_command shellcheck zero shellcheck "$repo_root/scripts/install.sh" "$repo_root/scripts/smoke-user-service.sh"
require_zero "ShellCheck"
capture_command diff-check zero git -C "$repo_root" diff --check
require_zero "git diff --check"

validation_dir="$smoke_root/install-validation"
mkdir -p -- "$validation_dir/owned-root"
chmod 700 "$validation_dir/owned-root"
cat >"$validation_dir/ubuntu-lts" <<'EOF'
ID=ubuntu
VERSION="24.04.4 LTS (Noble Numbat)"
EOF
cat >"$validation_dir/ubuntu-interim" <<'EOF'
ID=ubuntu
VERSION="25.10 (Questing Quokka)"
EOF
cat >"$validation_dir/non-ubuntu" <<'EOF'
ID=debian
VERSION="13 (trixie)"
EOF
validation_executable="$validation_dir/owned-root/smith"
printf '#!/bin/sh\n' >"$validation_executable"
chmod 700 "$validation_executable"
validation_runner="$smoke_root/run-install-validation.sh"
cat >"$validation_runner" <<'VALIDATION_RUNNER'
#!/usr/bin/env bash
set -euo pipefail
# shellcheck source=scripts/install.sh
source "$1"
shift
"$@"
VALIDATION_RUNNER
chmod 700 "$validation_runner"

capture_command validate-supported-host zero bash "$validation_runner" "$script_dir/install.sh" \
    validate_supported_host "$validation_dir/ubuntu-lts" "$EUID" x86_64
require_zero "supported-host validation"
capture_command validate-owned-root zero bash "$validation_runner" "$script_dir/install.sh" \
    validate_owned_safe_path "$validation_dir/owned-root" "test install root" "$EUID"
require_zero "owned-root validation"
capture_command validate-owned-executable zero bash "$validation_runner" "$script_dir/install.sh" \
    validate_owned_safe_path "$validation_executable" "test installed executable" "$EUID"
require_zero "owned-executable validation"

capture_command reject-non-lts nonzero bash "$validation_runner" "$script_dir/install.sh" \
    validate_supported_host "$validation_dir/ubuntu-interim" "$EUID" x86_64
require_nonzero_matching "non-LTS validation" 'Ubuntu LTS release'
capture_command reject-non-ubuntu nonzero bash "$validation_runner" "$script_dir/install.sh" \
    validate_supported_host "$validation_dir/non-ubuntu" "$EUID" x86_64
require_nonzero_matching "non-Ubuntu validation" 'expected Ubuntu LTS'
capture_command reject-effective-root nonzero bash "$validation_runner" "$script_dir/install.sh" \
    validate_supported_host "$validation_dir/ubuntu-lts" 0 x86_64
require_nonzero_matching "effective-root validation" 'refusing to install as effective root'
capture_command reject-wrong-owner nonzero bash "$validation_runner" "$script_dir/install.sh" \
    validate_owned_safe_path "$validation_dir/owned-root" "test install root" "$((EUID + 1))"
require_nonzero_matching "wrong-owner validation" 'must be owned by effective user'
chmod 720 "$validation_dir/owned-root"
capture_command reject-unsafe-root nonzero bash "$validation_runner" "$script_dir/install.sh" \
    validate_owned_safe_path "$validation_dir/owned-root" "test install root" "$EUID"
require_nonzero_matching "unsafe-root validation" 'must not be group- or world-writable'
chmod 700 "$validation_dir/owned-root"
chmod 702 "$validation_executable"
capture_command reject-unsafe-executable nonzero bash "$validation_runner" "$script_dir/install.sh" \
    validate_owned_safe_path "$validation_executable" "test installed executable" "$EUID"
require_nonzero_matching "unsafe-executable validation" 'must not be group- or world-writable'
chmod 700 "$validation_executable"
record "PASS: deterministic installer validation rejects non-LTS, root, wrong-owner, and unsafe modes"

build_mode="cargo-installer"
if [[ -n "${SMITH_SMOKE_BINARY:-}" ]]; then
    build_mode="supplied-binary"
    source_bin="$(readlink -f -- "$SMITH_SMOKE_BINARY")"
    [[ -f "$source_bin" && -x "$source_bin" ]] || fail "SMITH_SMOKE_BINARY is not executable"
    sha256sum "$source_bin" | awk '{print $1 "  supplied-smith"}' >"$run_dir/binary-input.sha256"
    capture_command copy-supplied-binary zero cp -- "$source_bin" "$smith_bin"
    require_zero "copy supplied binary"
    chmod 755 "$smith_bin"
    printf 'smith-install-root-v1\n' >"$install_root/.smith-install-root"
    chmod 600 "$install_root/.smith-install-root"
else
    capture_command source-installer zero bash "$repo_root/scripts/install.sh" --root "$install_root"
    require_zero "source installer"
fi

capture_command smith-help zero "$smith_bin" --help
require_zero "smith --help"
capture_command smith-version-subcommand zero "$smith_bin" version
require_zero "smith version"
capture_command smith-version-long zero "$smith_bin" --version
require_zero "smith --version"
capture_command smith-version-short zero "$smith_bin" -V
require_zero "smith -V"
grep -Fxq '0.4.0' "$run_dir/commands/$(printf '%03d' $((command_sequence - 2)))-smith-version-subcommand/output.log" || \
    fail "smith version did not report 0.4.0"
grep -Fxq 'smith 0.4.0' "$run_dir/commands/$(printf '%03d' $((command_sequence - 1)))-smith-version-long/output.log" || \
    fail "smith --version did not report smith 0.4.0"
grep -Fxq 'smith 0.4.0' "$LAST_OUTPUT" || fail "smith -V did not report smith 0.4.0"
sha256sum "$smith_bin" | awk '{print $1 "  installed-smith"}' >"$run_dir/installed-smith.sha256"
[[ ! -s "$manager_log" ]] || fail "installer/bootstrap attempted manager access through a sentinel"
record "PASS: fake managers blocked all manager access before lifecycle testing"

cat >"$fake_bin/systemctl" <<'FAKE_SYSTEMCTL'
#!/usr/bin/env bash
set -euo pipefail
log="${SMITH_SMOKE_MANAGER_LOG:?}"
state="${SMITH_SMOKE_MANAGER_STATE:?}"
entry="systemctl"
for arg in "$@"; do
    printf -v quoted '%q' "$arg"
    entry+=" $quoted"
done
printf '%s\n' "$entry" >>"$log"
(($# > 0)) || exit 64
[[ "$1" == "--user" ]] || { printf 'fake systemctl: --user is required\n' >&2; exit 97; }
shift
for arg in "$@"; do
    [[ "$arg" != "--now" ]] || { printf 'fake systemctl: combined --now operations are forbidden\n' >&2; exit 98; }
done
cmd="${1:-}"
[[ -n "$cmd" ]] || exit 64
shift
unit_path="${XDG_CONFIG_HOME:?}/systemd/user/smith.service"
active="$(cat "$state/active")"
substate="$(cat "$state/substate")"
enabled="$(cat "$state/enabled")"
result="$(cat "$state/result")"
restarts="$(cat "$state/restarts")"
main_code="$(cat "$state/main-code")"
main_status="$(cat "$state/main-status")"
malformed="$(cat "$state/malformed")"
multiline_enablement="$(cat "$state/multiline-enablement")"
[[ -f "$unit_path" ]] && load_state="loaded" || load_state="not-found"
case "$cmd" in
    daemon-reload) ;;
    start)
        [[ "$load_state" == "loaded" ]] || { printf 'Unit smith.service not found.\n' >&2; exit 5; }
        printf 'active\n' >"$state/active"; printf 'running\n' >"$state/substate"
        ;;
    restart)
        [[ "$load_state" == "loaded" ]] || { printf 'Unit smith.service not found.\n' >&2; exit 5; }
        printf 'active\n' >"$state/active"; printf 'running\n' >"$state/substate"
        ;;
    stop) printf 'inactive\n' >"$state/active"; printf 'dead\n' >"$state/substate" ;;
    enable)
        [[ "$load_state" == "loaded" ]] || { printf 'Unit smith.service not found.\n' >&2; exit 5; }
        printf 'enabled\n' >"$state/enabled"
        ;;
    disable) printf 'disabled\n' >"$state/enabled" ;;
    is-active)
        quiet=false; for arg in "$@"; do [[ "$arg" == "--quiet" ]] && quiet=true; done
        if [[ "$active" == "active" ]]; then $quiet || printf 'active\n'; exit 0; fi
        $quiet || printf '%s\n' "$active"; exit 3
        ;;
    is-enabled)
        quiet=false; for arg in "$@"; do [[ "$arg" == "--quiet" ]] && quiet=true; done
        if [[ "$multiline_enablement" == "yes" ]]; then $quiet || printf 'enabled\ndisabled\n'; exit 0; fi
        if [[ "$enabled" == "enabled" ]]; then $quiet || printf 'enabled\n'; exit 0; fi
        $quiet || printf '%s\n' "$enabled"; exit 1
        ;;
    show)
        if [[ "$malformed" == "yes" ]]; then
            printf 'LoadState=loaded\nLoadState=masked\nActiveState=garbage\nSubState=running\nResult=garbage\nNRestarts=NaN\nExecMainCode=exited\nExecMainStatus=999\n'
            exit 0
        fi
        properties=""; value_only=false
        while (($# > 0)); do
            case "$1" in
                --property=*) [[ -z "$properties" ]] || properties+=","; properties+="${1#--property=}" ;;
                --property|-p) shift; [[ -z "$properties" ]] || properties+=","; properties+="${1:-}" ;;
                --value) value_only=true ;;
            esac
            shift
        done
        [[ -n "$properties" ]] || properties="LoadState,ActiveState,SubState,UnitFileState"
        IFS=',' read -r -a property_list <<<"$properties"
        for property in "${property_list[@]}"; do
            case "$property" in
                LoadState) property_value="$load_state" ;;
                ActiveState) property_value="$active" ;;
                SubState) property_value="$substate" ;;
                UnitFileState) property_value="$enabled" ;;
                Result) property_value="$result" ;;
                NRestarts) property_value="$restarts" ;;
                ExecMainCode) property_value="$main_code" ;;
                ExecMainStatus) property_value="$main_status" ;;
                *) property_value="" ;;
            esac
            if $value_only; then printf '%s\n' "$property_value"; else printf '%s=%s\n' "$property" "$property_value"; fi
        done
        ;;
    cat) [[ -f "$unit_path" ]] || exit 1; cat "$unit_path" ;;
    status)
        printf 'LoadState=%s\nActiveState=%s\nSubState=%s\nUnitFileState=%s\n' "$load_state" "$active" "$substate" "$enabled"
        [[ "$active" == "active" ]]
        ;;
    list-unit-files) [[ "$load_state" != "loaded" ]] || printf 'smith.service %s\n' "$enabled" ;;
    *) printf 'fake systemctl: unsupported command: %s\n' "$cmd" >&2; exit 96 ;;
esac
FAKE_SYSTEMCTL
cat >"$fake_bin/loginctl" <<'FAKE_LOGINCTL'
#!/usr/bin/env bash
set -euo pipefail
log="${SMITH_SMOKE_MANAGER_LOG:?}"
state="${SMITH_SMOKE_MANAGER_STATE:?}"
entry="loginctl"
for arg in "$@"; do
    printf -v quoted '%q' "$arg"
    entry+=" $quoted"
done
printf '%s\n' "$entry" >>"$log"
case "${1:-}" in
    show-user)
        value_only=false; property=""; shift
        while (($# > 0)); do
            case "$1" in
                --property=*) property="${1#--property=}" ;;
                --property|-p) shift; property="${1:-}" ;;
                --value) value_only=true ;;
            esac
            shift
        done
        [[ -z "$property" || "$property" == "Linger" ]] || exit 64
        if [[ "$(cat "$state/multiline-linger")" == "yes" ]]; then
            if $value_only; then printf 'yes\nno\n'; else printf 'Linger=yes\nLinger=no\n'; fi
        elif $value_only; then printf 'no\n'; else printf 'Linger=no\n'; fi
        ;;
    enable-linger|disable-linger) printf 'fake loginctl: lingering mutation is forbidden\n' >&2; exit 98 ;;
    *) printf 'fake loginctl: unsupported command: %s\n' "${1:-}" >&2; exit 96 ;;
esac
FAKE_LOGINCTL
chmod 755 "$fake_bin/systemctl" "$fake_bin/loginctl"
hash -r
[[ "$(command -v systemctl)" == "$fake_bin/systemctl" ]] || fail "systemctl full fake is not first on PATH"
[[ "$(command -v loginctl)" == "$fake_bin/loginctl" ]] || fail "loginctl full fake is not first on PATH"

find_unmanaged_pids() {
    local proc_dir exe second_arg
    for proc_dir in /proc/[0-9]*; do
        [[ -r "$proc_dir/cmdline" ]] || continue
        exe="$(readlink "$proc_dir/exe" 2>/dev/null || true)"
        [[ "$exe" == "$smith_bin" || "$exe" == "$smith_bin (deleted)" ]] || continue
        second_arg="$(tr '\0' '\n' <"$proc_dir/cmdline" | sed -n '2p')"
        [[ "$second_arg" != "serve" ]] || printf '%s\n' "${proc_dir##*/}"
    done
}

cleanup_unmanaged() {
    local pid
    while IFS= read -r pid; do
        [[ -n "$pid" ]] || continue
        kill "$pid" 2>/dev/null || true
    done < <(find_unmanaged_pids)
}

process_monitor() {
    local cli_pid="$1" output_file="$2"
    while kill -0 "$cli_pid" 2>/dev/null; do
        find_unmanaged_pids >>"$output_file"
        sleep 0.01
    done
    find_unmanaged_pids >>"$output_file"
}

run_cli() {
    local label="$1" before after first cli_pid monitor_pid monitor_status rendered="" arg
    shift
    command_sequence=$((command_sequence + 1))
    CAPTURE_DIR="$(printf '%s/commands/%03d-%s' "$run_dir" "$command_sequence" "$label")"
    mkdir -p -- "$CAPTURE_DIR"
    for arg in "$smith_bin" "$@"; do printf -v rendered '%s %q' "$rendered" "$arg"; done
    printf '%s\n' "${rendered# }" >"$CAPTURE_DIR/command.txt"
    printf 'zero\n' >"$CAPTURE_DIR/expected.txt"
    : >"$CAPTURE_DIR/output.log"
    : >"$CAPTURE_DIR/unmanaged-pids.log"
    before="$(wc -l <"$manager_log")"
    "$smith_bin" "$@" >"$CAPTURE_DIR/output.log" 2>&1 &
    cli_pid=$!
    process_monitor "$cli_pid" "$CAPTURE_DIR/unmanaged-pids.log" &
    monitor_pid=$!
    if wait "$cli_pid"; then CAPTURE_STATUS=0; else CAPTURE_STATUS=$?; fi
    if wait "$monitor_pid"; then monitor_status=0; else monitor_status=$?; fi
    printf '%s\n' "$CAPTURE_STATUS" >"$CAPTURE_DIR/exit-status.txt"
    after="$(wc -l <"$manager_log")"; first=$((before + 1)); : >"$CAPTURE_DIR/manager.log"
    if ((after >= first)); then sed -n "${first},${after}p" "$manager_log" >"$CAPTURE_DIR/manager.log"; fi
    printf '%03d\t%s\t%s\tzero\n' "$command_sequence" "$label" "$CAPTURE_STATUS" >>"$command_results"
    record "$ ${rendered# }"
    tee -a "$transcript" <"$CAPTURE_DIR/output.log" >/dev/null
    tee -a "$transcript" <"$CAPTURE_DIR/manager.log" >/dev/null
    LAST_OUTPUT="$CAPTURE_DIR/output.log"; LAST_MANAGER_SEGMENT="$CAPTURE_DIR/manager.log"
    ((monitor_status == 0)) || fail "$label process monitor failed with exit status $monitor_status"
    if [[ -s "$CAPTURE_DIR/unmanaged-pids.log" ]]; then cleanup_unmanaged; fail "$label spawned an unmanaged 'smith serve' process"; fi
    ((CAPTURE_STATUS == 0)) || fail "$label failed with exit status $CAPTURE_STATUS"
}

assert_state() {
    local expected_active="$1" expected_enabled="$2" label="$3"
    [[ "$(cat "$manager_state/active")" == "$expected_active" ]] || fail "$label changed runtime state unexpectedly"
    [[ "$(cat "$manager_state/enabled")" == "$expected_enabled" ]] || fail "$label changed login enablement unexpectedly"
}

assert_segment_has() {
    local pattern="$1" label="$2"
    grep -Eq "$pattern" "$LAST_MANAGER_SEGMENT" || fail "$label did not issue the expected fake-manager command"
}

assert_segment_lacks_mutation() {
    local pattern="$1" label="$2"
    ! grep -Eq "$pattern" "$LAST_MANAGER_SEGMENT" || fail "$label combined lifecycle effects"
}

run_cli service-help settings service --help
for required in install start restart stop autostart uninstall; do
    grep -Eq "^[[:space:]]+$required([[:space:]]|$)" "$LAST_OUTPUT" || block "separated service command is missing: $required"
done

unit_path="$XDG_CONFIG_HOME/systemd/user/smith.service"
run_cli service-install settings service install
[[ -f "$unit_path" ]] || fail "service install did not stage $unit_path"
grep -Fq "$smith_bin" "$unit_path" || fail "unit does not use the isolated stable binary"
grep -Fxq 'StartLimitIntervalSec=300' "$unit_path" || fail "unit lacks bounded restart interval"
grep -Fxq 'StartLimitBurst=5' "$unit_path" || fail "unit lacks bounded restart burst"
grep -Eq '^[[:space:]]*RestartPreventExitStatus=78[[:space:]]*$' "$unit_path" || fail "unit retries permanent configuration failures"
! grep -Fq 'StartLimitAction=' "$unit_path" || fail "unit configures a disruptive start-limit action"
! grep -Fq 'After=default.target' "$unit_path" || fail "unit orders itself after its own login target"
cp -- "$unit_path" "$run_dir/rendered-smith.service"
cp -- "$smith_bin" "$run_dir/systemd-verify/smith"
chmod 500 "$run_dir/systemd-verify/smith"
python3 - "$run_dir/rendered-smith.service" "$run_dir/systemd-verify/smith.service" "$run_dir/systemd-verify/smith" <<'PY'
from pathlib import Path
import sys
source, destination, executable = map(Path, sys.argv[1:])
lines = source.read_text(encoding="utf-8").splitlines(keepends=True)
indexes = [index for index, line in enumerate(lines) if line.startswith("ExecStart=")]
if len(indexes) != 1:
    raise SystemExit(f"expected exactly one ExecStart, found {len(indexes)}")
lines[indexes[0]] = f'ExecStart="{executable}" serve\n'
destination.write_text("".join(lines), encoding="utf-8")
PY
diff -u "$run_dir/rendered-smith.service" "$run_dir/systemd-verify/smith.service" >"$run_dir/systemd-verify/transform.diff" || true
[[ "$(grep -c '^[-+]ExecStart=' "$run_dir/systemd-verify/transform.diff")" == "2" ]] || fail "replay unit transform changed more than ExecStart"
assert_state inactive disabled "service install"
assert_segment_lacks_mutation 'systemctl .* (start|restart|stop|enable|disable)( |$)' "service install"

command -v systemd-analyze >/dev/null 2>&1 || block "systemd-analyze is required for offline unit parsing"
capture_command systemd-version zero systemd-analyze --version
require_zero "systemd-analyze --version"
capture_command verify-original-unit zero systemd-analyze verify "$unit_path"
require_zero "systemd-analyze verify original unit"

run_cli status-installed settings service status
for field in 'unit|install|register|load' 'running|active|substate' 'enable|autostart|login' 'linger' 'bridge'; do
    grep -Eiq "$field" "$LAST_OUTPUT" || fail "status output is missing field family: $field"
done
grep -Eq 'bridge identity: not_listening' "$LAST_OUTPUT" || fail "status inherited an ambient bridge observation"
grep -Eq 'services\.ollama.*stopped' "$LAST_OUTPUT" || fail "status inherited an ambient Ollama observation"
assert_state inactive disabled "status"

printf 'activating\n' >"$manager_state/active"
printf 'auto-restart\n' >"$manager_state/substate"
printf 'exit-code\n' >"$manager_state/result"
printf '7\n' >"$manager_state/restarts"
printf '1\n' >"$manager_state/main-code"
printf '78\n' >"$manager_state/main-status"
run_cli status-restart-loop settings service status
grep -Eq 'supervisor availability: unavailable \(automatic restart pending\)' "$LAST_OUTPUT" || fail "restart loop was not classified as unavailable"
grep -Eq 'service result: exit-code' "$LAST_OUTPUT" || fail "status omitted service result"
grep -Eq 'restart attempts: 7' "$LAST_OUTPUT" || fail "status omitted restart count"
grep -Eq 'main process code: 1' "$LAST_OUTPUT" || fail "status omitted numeric main process code"
grep -Eq 'main process status: 78' "$LAST_OUTPUT" || fail "status omitted main exit status"
grep -Eq 'recovery: .*service stop' "$LAST_OUTPUT" || fail "status omitted restart-loop recovery guidance"
assert_state activating disabled "restart-loop status"

printf 'yes\n' >"$manager_state/malformed"
run_cli status-malformed-manager settings service status
grep -Eq 'load state: unknown' "$LAST_OUTPUT" || fail "duplicate load state was trusted"
grep -Eq 'active state: unknown' "$LAST_OUTPUT" || fail "malformed active state was trusted"
grep -Eq 'service result: unknown' "$LAST_OUTPUT" || fail "malformed service result was trusted"
grep -Eq 'restart attempts: unknown' "$LAST_OUTPUT" || fail "malformed restart count was trusted"
grep -Eq 'main process code: unknown' "$LAST_OUTPUT" || fail "textual main process code was trusted"
grep -Eq 'main process status: unknown' "$LAST_OUTPUT" || fail "out-of-range main process status was trusted"
printf 'no\n' >"$manager_state/malformed"

printf 'yes\n' >"$manager_state/multiline-enablement"
printf 'yes\n' >"$manager_state/multiline-linger"
run_cli status-contradictory-manager settings service status
grep -Eq 'enabled at login: unknown' "$LAST_OUTPUT" || fail "contradictory enablement output was trusted"
grep -Eq 'unit enablement state: unknown' "$LAST_OUTPUT" || fail "contradictory raw enablement was trusted"
grep -Eq 'linger: unknown' "$LAST_OUTPUT" || fail "contradictory linger output was trusted"
printf 'no\n' >"$manager_state/multiline-enablement"
printf 'no\n' >"$manager_state/multiline-linger"

printf 'inactive\n' >"$manager_state/active"
printf 'dead\n' >"$manager_state/substate"
printf 'success\n' >"$manager_state/result"
printf '0\n' >"$manager_state/restarts"
printf '1\n' >"$manager_state/main-code"
printf '0\n' >"$manager_state/main-status"
printf 'enabled-runtime\n' >"$manager_state/enabled"
run_cli status-runtime-enabled settings service status
grep -Eq 'enabled at login: no' "$LAST_OUTPUT" || fail "runtime-only enablement was reported as enabled at login"
grep -Eq 'unit enablement state: enabled-runtime' "$LAST_OUTPUT" || fail "raw runtime-only enablement was not preserved"
printf 'disabled\n' >"$manager_state/enabled"

run_cli service-start settings service start
assert_segment_has 'systemctl .* start( |.* )smith\.service' "service start"
assert_segment_lacks_mutation 'systemctl .* (enable|disable|restart|stop)( |$)' "service start"
assert_state active disabled "service start"
run_cli service-restart settings service restart
assert_segment_has 'systemctl .* restart( |.* )smith\.service' "service restart"
assert_segment_lacks_mutation 'systemctl .* (enable|disable|start|stop)( |$)' "service restart"
assert_state active disabled "service restart"
run_cli service-stop settings service stop
assert_segment_has 'systemctl .* stop( |.* )smith\.service' "service stop"
assert_segment_lacks_mutation 'systemctl .* (enable|disable|start|restart)( |$)' "service stop"
assert_state inactive disabled "service stop"
run_cli autostart-on settings service autostart on
assert_segment_has 'systemctl .* enable( |.* )smith\.service' "autostart on"
assert_segment_lacks_mutation 'systemctl .* (start|restart|stop|disable)( |$)' "autostart on"
assert_state inactive enabled "autostart on"
run_cli autostart-off settings service autostart off
assert_segment_has 'systemctl .* disable( |.* )smith\.service' "autostart off"
assert_segment_lacks_mutation 'systemctl .* (start|restart|stop|enable)( |$)' "autostart off"
assert_state inactive disabled "autostart off"
run_cli status-final settings service status
assert_state inactive disabled "final status"
run_cli service-uninstall settings service uninstall
[[ ! -e "$unit_path" ]] || fail "service uninstall left the unit installed"
assert_segment_has 'systemctl .* stop( |.* )smith\.service' "service uninstall"
assert_segment_has 'systemctl .* disable( |.* )smith\.service' "service uninstall"
assert_segment_has 'systemctl .* daemon-reload( |$)' "service uninstall"
assert_state inactive disabled "service uninstall"

! grep -Eq 'systemctl .*enable .*--now|systemctl .*--now .*enable' "$manager_log" || fail "combined enable --now was attempted"
! grep -Eq '^loginctl +(enable-linger|disable-linger)( |$)' "$manager_log" || fail "a lingering mutation was attempted"
[[ -z "$(find_unmanaged_pids)" ]] || { cleanup_unmanaged; fail "an unmanaged Smith supervisor survived"; }
grep -Eq '^loginctl +show-user( |$)' "$manager_log" || fail "status did not inspect lingering state"
record "PASS: sandbox lifecycle remained separated with no enable --now, linger mutation, or unmanaged spawn"

rm -rf -- "$smoke_root"
sandbox_removed="yes"
[[ ! -e "$smoke_root" ]] || fail "sandbox cleanup did not remove the temporary executable"
capture_command verify-retained-unit-after-cleanup zero systemd-analyze verify "$run_dir/systemd-verify/smith.service"
require_zero "post-cleanup retained unit verification"
post_cleanup_unit_verify="pass"
record "PASS: retained unit fixture parses after sandbox executable cleanup"
overall_result="pass"
}

finalize() {
    local original_status=$? final_status finished_at_utc source_inputs_after_sha256 cargo_offline_requested artifact_manifest_tmp
    trap - EXIT HUP INT TERM
    set +e
    final_status="$original_status"
    if [[ "${overall_result:-pending}" == "blocked" ]]; then final_status=2; fi
    if [[ "${overall_result:-pending}" == "fail" && "$final_status" == "0" ]]; then final_status=1; fi
    if [[ -d "${smoke_root:-}" ]]; then
        if declare -F cleanup_unmanaged >/dev/null; then cleanup_unmanaged; fi
        rm -rf -- "$smoke_root"
    fi
    if [[ ! -e "${smoke_root:-/nonexistent}" ]]; then sandbox_removed="yes"; else sandbox_removed="no"; fi
    (
        cd -- "$repo_root" || exit 1
        git status --porcelain=v2 --branch >"$run_dir/repository/status.after.txt"
        git diff --binary --full-index HEAD -- >"$run_dir/repository/dirty.after.patch"
        capture_source_inputs "$run_dir/repository/source-inputs.after.sha256z"
    )
    source_inputs_after_sha256="$(sha256sum "$run_dir/repository/source-inputs.after.sha256z" | awk '{print $1}')"
    source_inputs_stable="yes"
    cmp -s "$run_dir/repository/dirty.before.patch" "$run_dir/repository/dirty.after.patch" || source_inputs_stable="no"
    cmp -s "$run_dir/repository/source-inputs.before.sha256z" "$run_dir/repository/source-inputs.after.sha256z" || source_inputs_stable="no"
    if [[ "$source_inputs_stable" != "yes" ]]; then
        overall_result="fail"
        final_status=1
    elif [[ "${overall_result:-pending}" == "pending" ]]; then
        if ((final_status == 0)); then overall_result="pass"; else overall_result="fail"; fi
    fi
    finished_at_utc="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    cargo_offline_requested="no"
    [[ "${CARGO_NET_OFFLINE:-false}" == "true" ]] && cargo_offline_requested="yes"
    cat >"$run_dir/result.env" <<EOF
schema=smith-user-service-smoke/v2
overall_result=$overall_result
exit_status=$final_status
sandbox_removed=$sandbox_removed
rendered_unit=$([[ -f "$run_dir/rendered-smith.service" ]] && printf produced || printf not_produced)
post_cleanup_unit_verify=${post_cleanup_unit_verify:-not_run}
source_inputs_stable=$source_inputs_stable
EOF
    cat >"$run_dir/environment.txt" <<EOF
manager_isolation=PATH-shadowed fake systemctl/loginctl activated before installer code; invalid user bus; ambient bridge/Docker probes suppressed
manager_isolation_limit=no syscall sandbox; deliberately absolute manager paths are outside this smoke contract
cargo_locked=yes
cargo_offline_requested=$cargo_offline_requested
cargo_network_hermetic=no
cargo_cache_isolated=no
build_mode=${build_mode:-not_started}
checksum_integrity=detects stale mixing and accidental changes; owner can rewrite package and checksum
external_digest_anchor=required in independent review or approval record
supplied_binary_provenance=$([[ "${build_mode:-}" == supplied-binary ]] && printf external-digest-only || printf not-applicable)
EOF
    cat >"$run_dir/validation-manifest.txt" <<EOF
schema=smith-user-service-smoke/v2
work_record=$WORK_RECORD
repository=smith
producer_context=Zed coding agent GPT-5.6-Sol
independent_reviewer=pending
started_at_utc=$started_at_utc
finished_at_utc=$finished_at_utc
branch=$branch
base_revision=$PLAN_BASE_REVISION
head_revision=$head_revision
head_tree=$head_tree
worktree_dirty=$worktree_dirty
dirty_diff_sha256=$dirty_diff_sha256
source_inputs_before_sha256=$source_inputs_before_sha256
source_inputs_after_sha256=$source_inputs_after_sha256
source_inputs_stable=$source_inputs_stable
build_mode=${build_mode:-not_started}
manager_isolation=path-shadowed-fakes-plus-invalid-user-bus
cargo_locked=yes
cargo_offline_requested=$cargo_offline_requested
cargo_network_hermetic=no
cargo_cache_isolated=no
checksum_integrity=owner-controlled; external digest anchor required
review_anchor_status=pending
overall_result=$overall_result
exit_status=$final_status
sandbox_removed=$sandbox_removed
post_cleanup_unit_verify=${post_cleanup_unit_verify:-not_run}
EOF
    artifact_manifest_tmp="${run_dir}.artifacts.sha256.tmp"
    (
        cd -- "$run_dir" || exit 1
        find . -type f -print0 | LC_ALL=C sort -z | xargs -0 sha256sum >"$artifact_manifest_tmp"
    )
    mv -- "$artifact_manifest_tmp" "$run_dir/artifacts.sha256"
    chmod -R a-w -- "$run_dir"
    if ((final_status != 0)); then
        printf '\n--- sandbox smoke transcript (failure) ---\n' >&2
        cat "$transcript" >&2
        printf '%s\n' '--- end transcript ---' >&2
    fi
    printf 'sandbox evidence retained in %s\n' "$run_dir"
    exit "$final_status"
}

sandbox_removed="no"
post_cleanup_unit_verify="not_run"
overall_result="pending"
source_inputs_stable="unknown"
worktree_dirty="unknown"
dirty_diff_sha256="unknown"
source_inputs_before_sha256="unknown"
trap finalize EXIT
trap 'overall_result="interrupted"; exit 129' HUP
trap 'overall_result="interrupted"; exit 130' INT
trap 'overall_result="interrupted"; exit 143' TERM

main_smoke

printf '\n--- sandbox smoke transcript ---\n'
cat "$transcript"
printf '%s\n' '--- end transcript ---'
