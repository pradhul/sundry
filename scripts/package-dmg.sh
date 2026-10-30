#!/usr/bin/env bash
# Build a shareable Sundry.dmg (ad-hoc signed — free; the other Mac may need
# right-click → Open the first time).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MAC="$ROOT/sundry-mac"
DIST="$ROOT/dist"
STAGING="$DIST/dmg-staging"
DERIVED="$DIST/DerivedData"
VERSION="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$MAC/Sundry/Info.plist" 2>/dev/null || echo "0.1.0")"
DMG_NAME="Sundry-${VERSION}.dmg"
APP_NAME="Sundry.app"

export DEVELOPER_DIR="${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}"
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"
unset CARGO_TARGET_DIR

echo "==> Building Rust core (static lib)"
cd "$ROOT"
cargo build -p sundry-ffi --release --target aarch64-apple-darwin

echo "==> Building Sundry (Release)"
rm -rf "$DERIVED"
xcodebuild \
  -project "$MAC/Sundry.xcodeproj" \
  -scheme Sundry \
  -configuration Release \
  -destination 'platform=macOS,arch=arm64' \
  -derivedDataPath "$DERIVED" \
  CODE_SIGN_IDENTITY="-" \
  CODE_SIGNING_ALLOWED=YES \
  CODE_SIGNING_REQUIRED=NO \
  build

APP_SRC="$DERIVED/Build/Products/Release/$APP_NAME"
if [[ ! -d "$APP_SRC" ]]; then
  echo "error: Release app not found at $APP_SRC" >&2
  exit 1
fi

echo "==> Staging DMG contents"
rm -rf "$STAGING" "$DIST/$DMG_NAME" "$DIST/rw.$DMG_NAME"
mkdir -p "$STAGING"
ditto "$APP_SRC" "$STAGING/$APP_NAME"
ln -s /Applications "$STAGING/Applications"

# Ad-hoc sign the bundled app so Gatekeeper has *something* to check.
echo "==> Ad-hoc codesign"
codesign --force --deep --sign - \
  --entitlements "$MAC/Sundry/Sundry.entitlements" \
  --options runtime \
  "$STAGING/$APP_NAME" || codesign --force --deep --sign - "$STAGING/$APP_NAME"

# Sanity: no absolute deps on this machine's Rust target dir.
# otool prints the binary path as a header — only check indented dependency lines.
if otool -L "$STAGING/$APP_NAME/Contents/MacOS/Sundry" | awk 'NR>1 && /\/Users\/|\/target\//' | grep -q .; then
  echo "error: binary still references a local absolute library path — won't run on another Mac" >&2
  otool -L "$STAGING/$APP_NAME/Contents/MacOS/Sundry" | awk 'NR>1 && /\/Users\/|\/target\//'
  exit 1
fi

echo "==> Creating $DMG_NAME"
mkdir -p "$DIST"
# Optional volume icon
if [[ -f "$MAC/Sundry/Resources/AppIcon.icns" ]]; then
  cp "$MAC/Sundry/Resources/AppIcon.icns" "$STAGING/.VolumeIcon.icns"
fi
hdiutil create \
  -volname "Sundry" \
  -srcfolder "$STAGING" \
  -ov \
  -format UDZO \
  "$DIST/$DMG_NAME"

rm -rf "$STAGING"

echo
echo "Packed: $DIST/$DMG_NAME"
echo "AirDrop that file. On the other Mac: open the DMG, drag Sundry → Applications,"
echo "then right-click Sundry → Open the first time (ad-hoc signed, no \$99 cert)."
