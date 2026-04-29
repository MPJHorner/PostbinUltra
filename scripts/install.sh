#!/usr/bin/env bash
# Postbin Ultra one-liner installer.
#
#   curl -sSL https://raw.githubusercontent.com/MPJHorner/PostbinUltra/main/scripts/install.sh | bash
#
# Detects OS + arch, downloads the matching release artefact from GitHub,
# installs it to the right place, and prints how to launch.
#
# Override the install location with $PBU_INSTALL_DIR. On macOS the default
# is /Applications (drag-and-drop equivalent); on Linux it's ~/.local/bin.
#
# This is best-effort: if anything fails, fall back to the manual install
# instructions at https://mpjhorner.github.io/PostbinUltra/install/

set -euo pipefail

REPO="MPJHorner/PostbinUltra"
RELEASES="https://github.com/${REPO}/releases"

err() { printf '\033[0;31m%s\033[0m\n' "$*" >&2; }
say() { printf '\033[0;32m%s\033[0m\n' "$*"; }
note() { printf '\033[0;36m%s\033[0m\n' "$*"; }

require() {
    if ! command -v "$1" >/dev/null 2>&1; then
        err "missing dependency: $1"
        exit 1
    fi
}

require curl
require uname

# ── Detect platform ──────────────────────────────────────────────
os="$(uname -s)"
arch="$(uname -m)"

case "$os" in
    Darwin) target_os="apple-darwin" ;;
    Linux)  target_os="unknown-linux-gnu" ;;
    *)
        err "Unsupported OS: $os"
        err "Use the manual install instructions at https://mpjhorner.github.io/PostbinUltra/install/"
        exit 1
        ;;
esac

case "$arch" in
    x86_64|amd64)   target_arch="x86_64" ;;
    arm64|aarch64)  target_arch="aarch64" ;;
    *)
        err "Unsupported arch: $arch"
        exit 1
        ;;
esac

target="${target_arch}-${target_os}"

# ── Resolve latest version ───────────────────────────────────────
say "==> Detecting latest release"
version="$(curl -sSL -o /dev/null -w '%{url_effective}' "${RELEASES}/latest" | sed 's|.*/v||')"
if [[ -z "$version" ]]; then
    err "Could not determine latest release version. Check ${RELEASES} manually."
    exit 1
fi
say "    Latest: v${version}"
say "    Target: ${target}"

# ── Download + install ───────────────────────────────────────────
case "$os" in
    Darwin)
        artefact="PostbinUltra-${version}-${target}.dmg"
        url="${RELEASES}/download/v${version}/${artefact}"
        install_dir="${PBU_INSTALL_DIR:-/Applications}"
        tmp="$(mktemp -d)"
        trap 'rm -rf "$tmp"' EXIT

        say "==> Downloading ${artefact}"
        curl -fSL --progress-bar -o "${tmp}/${artefact}" "${url}"

        say "==> Mounting .dmg"
        mountpoint="$(hdiutil attach "${tmp}/${artefact}" -nobrowse -noautoopen | awk '/Volumes/ {print $NF}')"
        if [[ -z "$mountpoint" ]]; then
            err "Could not mount the .dmg. Open it manually from ${tmp}/${artefact}."
            exit 1
        fi

        say "==> Installing PostbinUltra.app to ${install_dir}"
        # macOS `cp -R src dst/` only puts `src` as `dst/src` when `dst` is an
        # existing directory. If we created the install dir on the fly (rare —
        # /Applications always exists), the cp would instead make `dst` a copy
        # of src's contents. Guard with mkdir -p.
        mkdir -p "${install_dir}"
        if [[ -d "${install_dir}/PostbinUltra.app" ]]; then
            note "    Removing existing ${install_dir}/PostbinUltra.app"
            rm -rf "${install_dir}/PostbinUltra.app"
        fi
        cp -R "${mountpoint}/PostbinUltra.app" "${install_dir}/PostbinUltra.app"
        hdiutil detach -quiet "${mountpoint}"

        # Clear quarantine so Gatekeeper doesn't block the launch.
        # Users still see the "unverified developer" dialog the first time
        # if signing is off; this just shaves one click off.
        xattr -dr com.apple.quarantine "${install_dir}/PostbinUltra.app" 2>/dev/null || true

        say ""
        say "✓ Installed to ${install_dir}/PostbinUltra.app"
        say ""
        note "Launch:  open '${install_dir}/PostbinUltra.app'"
        note "Or:      Spotlight → 'Postbin Ultra'"
        ;;

    Linux)
        artefact="PostbinUltra-${version}-${target}.tar.gz"
        url="${RELEASES}/download/v${version}/${artefact}"
        install_dir="${PBU_INSTALL_DIR:-${HOME}/.local/bin}"
        tmp="$(mktemp -d)"
        trap 'rm -rf "$tmp"' EXIT

        say "==> Downloading ${artefact}"
        curl -fSL --progress-bar -o "${tmp}/${artefact}" "${url}"

        say "==> Extracting"
        tar -xzf "${tmp}/${artefact}" -C "${tmp}"

        say "==> Installing PostbinUltra to ${install_dir}"
        mkdir -p "${install_dir}"
        cp "${tmp}/PostbinUltra" "${install_dir}/PostbinUltra"
        chmod +x "${install_dir}/PostbinUltra"

        say ""
        say "✓ Installed to ${install_dir}/PostbinUltra"
        say ""
        if ! echo "$PATH" | tr ':' '\n' | grep -qx "${install_dir}"; then
            note "Add ${install_dir} to your \$PATH:"
            note "  echo 'export PATH=\"${install_dir}:\$PATH\"' >> ~/.bashrc"
        fi
        note "Launch:  PostbinUltra"
        ;;
esac
