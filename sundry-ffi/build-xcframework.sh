#!/usr/bin/env bash
# sundry-ffi/build-xcframework.sh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# Respect an existing CARGO_TARGET_DIR (Cursor sandboxes set one).
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"

source "$HOME/.cargo/env"
rustup target add aarch64-apple-darwin >/dev/null
cargo build -p sundry-ffi --release --target aarch64-apple-darwin

LIB="$TARGET_DIR/aarch64-apple-darwin/release/libsundry_ffi.dylib"
A_LIB="$TARGET_DIR/aarch64-apple-darwin/release/libsundry_ffi.a"
OUT="$ROOT/target/xcframework"
rm -rf "$OUT" && mkdir -p "$OUT/headers" "$OUT/swift"

cargo run -p sundry-ffi --bin uniffi-bindgen -- generate \
  --library "$LIB" \
  --language swift --out-dir "$OUT/swift"

cp "$OUT"/swift/*.swift "$ROOT/sundry-ffi/generated/" 2>/dev/null || true
cp "$OUT"/swift/*.h "$OUT/headers/"
cp "$OUT"/swift/*.modulemap "$OUT/headers/module.modulemap"

if command -v xcodebuild >/dev/null 2>&1 && xcodebuild -version >/dev/null 2>&1; then
  xcodebuild -create-xcframework \
    -library "$A_LIB" \
    -headers "$OUT/headers" \
    -output "$OUT/Sundry.xcframework"
  echo "Built $OUT/Sundry.xcframework"
else
  echo "xcodebuild (full Xcode) not available; static lib is at $A_LIB"
fi

echo "Swift bindings: $OUT/swift/sundry_ffi.swift"
