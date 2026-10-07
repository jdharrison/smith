#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: scripts/install.sh [--root INSTALL_ROOT]

Build and install the reviewed Smith checkout into INSTALL_ROOT.
The default is SMITH_INSTALL_ROOT when set, otherwise $HOME/.local.
This script installs only the Smith binary; it does not configure services,
sidecars, Docker, or user lingering.
EOF
}

fail() {
    printf 'install: %s\n' "$*" >&2
    exit 1
}

os_release_value() {
    local file="$1" key="$2" value
    value="$(sed -n "s/^${key}=//p" "$file" | sed -n '1p')"
    value="${value%\"}"
    value="${value#\"}"
    value="${value%\'}"
    value="${value#\'}"
    printf '%s\n' "$value"
}

validate_supported_host() {
    local os_release="$1" effective_uid="$2" architecture="$3"
    local os_id version

    [[ "$effective_uid" =~ ^[0-9]+$ ]] || fail "cannot determine effective user ID"
    ((effective_uid != 0)) || fail "refusing to install as effective root; run as the intended unprivileged Smith user"
    [[ "$architecture" == "x86_64" ]] || fail "unsupported architecture: expected x86_64"
    [[ -r "$os_release" ]] || fail "cannot verify Ubuntu LTS: $os_release is unavailable"

    os_id="$(os_release_value "$os_release" ID)"
    version="$(os_release_value "$os_release" VERSION)"
    [[ "$os_id" == "ubuntu" ]] || fail "unsupported operating system: expected Ubuntu LTS"
    [[ "$version" =~ (^|[[:space:]])LTS([[:space:]]|$) ]] || \
        fail "unsupported Ubuntu release: VERSION must identify an Ubuntu LTS release"
}

validate_owned_safe_path() {
    local path="$1" label="$2" effective_uid="$3"
    local owner mode permissions group_digit world_digit

    owner="$(stat -Lc '%u' -- "$path")" || fail "cannot inspect $label ownership: $path"
    mode="$(stat -Lc '%a' -- "$path")" || fail "cannot inspect $label permissions: $path"
    [[ "$owner" == "$effective_uid" ]] || \
        fail "$label must be owned by effective user $effective_uid: $path (owner $owner)"
    permissions="${mode: -3}"
    [[ "$permissions" =~ ^[0-7]{3}$ ]] || fail "cannot interpret $label permissions '$mode': $path"
    group_digit="${permissions:1:1}"
    world_digit="${permissions:2:1}"
    (((group_digit & 2) == 0 && (world_digit & 2) == 0)) || \
        fail "$label must not be group- or world-writable: $path (mode $mode)"
}

main() {
    local install_root="" os_release architecture effective_uid script_dir repo_root smith_bin marker marker_tmp

    effective_uid="$EUID"
    os_release="/etc/os-release"
    while (($# > 0)); do
        case "$1" in
            --root)
                (($# >= 2)) || fail "--root requires a path"
                install_root="$2"
                shift 2
                ;;
            -h|--help)
                usage
                exit 0
                ;;
            *)
                fail "unknown argument: $1"
                ;;
        esac
    done

    if [[ -z "$install_root" ]]; then
        if [[ -n "${SMITH_INSTALL_ROOT:-}" ]]; then
            install_root="$SMITH_INSTALL_ROOT"
        elif [[ -n "${HOME:-}" ]]; then
            install_root="$HOME/.local"
        else
            fail "HOME is unset; pass --root or set SMITH_INSTALL_ROOT"
        fi
    fi

    [[ "$install_root" == /* ]] || fail "install root must be an absolute path: $install_root"

    command -v uname >/dev/null 2>&1 || fail "uname is required"
    command -v stat >/dev/null 2>&1 || fail "GNU stat is required"
    command -v readlink >/dev/null 2>&1 || fail "readlink is required"
    architecture="$(uname -m)"
    validate_supported_host "$os_release" "$effective_uid" "$architecture"

    command -v cargo >/dev/null 2>&1 || fail "cargo is required"

    script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
    repo_root="$(cd -- "$script_dir/.." && pwd -P)"
    [[ -f "$repo_root/Cargo.toml" ]] || fail "Cargo.toml not found at repository root: $repo_root"

    mkdir -p -- "$install_root"
    install_root="$(cd -- "$install_root" && pwd -P)"
    [[ -w "$install_root" ]] || fail "install root is not writable: $install_root"
    validate_owned_safe_path "$install_root" "canonical install root" "$effective_uid"

    printf 'Installing Smith from %s to %s\n' "$repo_root" "$install_root"
    cargo install --locked --path "$repo_root" --root "$install_root"

    smith_bin="$install_root/bin/smith"
    [[ -f "$smith_bin" ]] || fail "cargo completed but did not create $smith_bin"
    smith_bin="$(readlink -f -- "$smith_bin")"
    [[ "$smith_bin" == "$install_root/bin/smith" ]] || \
        fail "installed binary resolves outside the canonical install root: $smith_bin"
    chmod 0755 -- "$smith_bin"
    [[ -x "$smith_bin" ]] || fail "installed binary is not executable: $smith_bin"
    validate_owned_safe_path "$smith_bin" "canonical installed executable" "$effective_uid"
    "$smith_bin" --help >/dev/null
    "$smith_bin" --version >/dev/null

    marker="$install_root/.smith-install-root"
    marker_tmp="$install_root/.smith-install-root.tmp.$$"
    (umask 077 && printf 'smith-install-root-v1\n' >"$marker_tmp")
    chmod 0600 -- "$marker_tmp"
    mv -f -- "$marker_tmp" "$marker"
    validate_owned_safe_path "$marker" "Smith install marker" "$effective_uid"

    printf '\nInstalled and verified: %s\n' "$smith_bin"
    printf 'No service, sidecar, Docker container, or lingering setting was changed.\n'
    printf '\nNext commands (each effect is separate):\n'
    printf '  %q settings service install\n' "$smith_bin"
    printf '  %q settings service status\n' "$smith_bin"
    printf '  %q settings service start\n' "$smith_bin"
    printf '  %q settings service status\n' "$smith_bin"
    printf '\nOptional login autostart (does not start Smith now):\n'
    printf '  %q settings service autostart on\n' "$smith_bin"
    printf '\nTo use smith by name in this shell:\n'
    printf '  export PATH=%q:%s\n' "$install_root/bin" "\$PATH"
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
    main "$@"
fi
