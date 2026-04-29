#!/usr/bin/env bash
#
# Assembles a macOS .app bundle and a distributable .dmg for the Postbin Ultra
# desktop app, using only macOS-native tools (no Tauri, no cargo-bundle, no
# Node dependencies).
#
# Output:
#   target/bundle/PostbinUltra.app
#   target/bundle/PostbinUltra-<version>.dmg
#
# Usage:
#   scripts/bundle-mac.sh                    # builds release binary first
#   scripts/bundle-mac.sh --skip-build       # uses an existing release binary
#   scripts/bundle-mac.sh --no-dmg           # only assemble the .app
#
# Run from the repo root.

set -euo pipefail

APP_NAME="PostbinUltra"
BIN_NAME="PostbinUltra"
BUNDLE_ID="co.uk.matthorner.postbin-ultra"
COPYRIGHT="Copyright © 2026 MPJHorner."
ICON_SRC="crates/postbin-ultra-desktop/assets/icons/AppIcon.icns"
ICONSET_SRC="crates/postbin-ultra-desktop/assets/icons/AppIcon.iconset"

skip_build=false
make_dmg=true
for arg in "$@"; do
    case "$arg" in
        --skip-build) skip_build=true ;;
        --no-dmg) make_dmg=false ;;
        -h|--help)
            sed -n '2,18p' "$0"
            exit 0
            ;;
        *)
            echo "unknown flag: $arg" >&2
            exit 2
            ;;
    esac
done

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "scripts/bundle-mac.sh only runs on macOS." >&2
    exit 1
fi

CARGO="${CARGO:-cargo}"
if ! command -v "$CARGO" >/dev/null 2>&1; then
    CARGO="$HOME/.cargo/bin/cargo"
fi

# Pull the version from the desktop crate's Cargo.toml so the .dmg name
# matches the release that's about to ship. `awk -F'"'` splits on quotes which
# is the right shape for `version = "1.1.0"`.
VERSION="$(awk -F'"' '/^version *=/ {print $2; exit}' crates/postbin-ultra-desktop/Cargo.toml)"
if [[ -z "$VERSION" ]]; then
    echo "could not read version from crates/postbin-ultra-desktop/Cargo.toml" >&2
    exit 1
fi

if [[ ! -f "$ICON_SRC" ]]; then
    echo "icon set missing — running icon-gen + iconutil" >&2
    "$CARGO" run -p icon-gen
    iconutil -c icns "$ICONSET_SRC" -o "$ICON_SRC"
fi

if [[ "$skip_build" != true ]]; then
    echo "→ building release binary"
    "$CARGO" build --release -p postbin-ultra-desktop
fi

BIN_PATH="target/release/$BIN_NAME"
if [[ ! -x "$BIN_PATH" ]]; then
    echo "missing release binary at $BIN_PATH" >&2
    exit 1
fi

OUT_DIR="target/bundle"
APP_DIR="$OUT_DIR/$APP_NAME.app"
CONTENTS="$APP_DIR/Contents"
MACOS_DIR="$CONTENTS/MacOS"
RESOURCES_DIR="$CONTENTS/Resources"

rm -rf "$APP_DIR"
mkdir -p "$MACOS_DIR" "$RESOURCES_DIR"

cp "$BIN_PATH" "$MACOS_DIR/$BIN_NAME"
chmod +x "$MACOS_DIR/$BIN_NAME"
cp "$ICON_SRC" "$RESOURCES_DIR/AppIcon.icns"

cat > "$CONTENTS/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>$APP_NAME</string>
    <key>CFBundleDisplayName</key><string>Postbin Ultra</string>
    <key>CFBundleIdentifier</key><string>$BUNDLE_ID</string>
    <key>CFBundleExecutable</key><string>$BIN_NAME</string>
    <key>CFBundleIconFile</key><string>AppIcon</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>$VERSION</string>
    <key>CFBundleVersion</key><string>$VERSION</string>
    <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
    <key>LSMinimumSystemVersion</key><string>11.0</string>
    <key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSHumanReadableCopyright</key><string>$COPYRIGHT</string>
    <key>NSSupportsAutomaticGraphicsSwitching</key><true/>
</dict>
</plist>
PLIST

# Validate the plist so a typo in this script can't ship a broken bundle.
plutil -lint "$CONTENTS/Info.plist" >/dev/null

echo "→ built $APP_DIR"

if [[ "$make_dmg" == true ]]; then
    DMG_NAME="$APP_NAME-$VERSION.dmg"
    DMG_PATH="$OUT_DIR/$DMG_NAME"
    rm -f "$DMG_PATH"

    STAGING="$(mktemp -d)"
    trap 'rm -rf "$STAGING"' EXIT
    cp -R "$APP_DIR" "$STAGING/$APP_NAME.app"
    ln -s /Applications "$STAGING/Applications"

    hdiutil create \
        -volname "$APP_NAME" \
        -srcfolder "$STAGING" \
        -ov \
        -format UDZO \
        "$DMG_PATH" >/dev/null
    echo "→ built $DMG_PATH"
fi

echo "done."
