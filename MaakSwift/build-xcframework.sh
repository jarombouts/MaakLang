#!/usr/bin/env bash
# Build schildpad-ffi for iOS device + simulator and bundle a SchildpadFFI.xcframework
# that the Maak SwiftUI app links. Re-run whenever core/ or ffi-c/ changes.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
export PATH="$HOME/.cargo/bin:$PATH"

INCLUDE="MaakSwift/SchildpadFFI/include"
OUT="MaakSwift/SchildpadFFI/SchildpadFFI.xcframework"
LIB="libschildpad_ffi.a"

echo "==> building schildpad-ffi for iOS device + simulator (release)"
cargo build --release -p schildpad-ffi --target aarch64-apple-ios
cargo build --release -p schildpad-ffi --target aarch64-apple-ios-sim

echo "==> assembling $OUT"
rm -rf "$OUT"
xcodebuild -create-xcframework \
  -library "target/aarch64-apple-ios/release/$LIB"     -headers "$INCLUDE" \
  -library "target/aarch64-apple-ios-sim/release/$LIB" -headers "$INCLUDE" \
  -output "$OUT"

echo "==> done: $OUT"
