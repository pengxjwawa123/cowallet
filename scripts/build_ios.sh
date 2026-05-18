#!/usr/bin/env bash
set -euo pipefail

# Build CoWallet iOS app (Rust FFI + Flutter)
# Usage: ./scripts/build_ios.sh [debug|release|profile]
#        ./scripts/build_ios.sh release --no-codesign   (for CI)

MODE="${1:-release}"
shift || true
EXTRA_ARGS="$*"

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
RUST_CRATE="$PROJECT_ROOT/crates/ffi-mobile"
MOBILE_DIR="$PROJECT_ROOT/mobile"

# iOS deployment target (must be >= 12.0 for Flutter and aws-lc-sys)
export IPHONEOS_DEPLOYMENT_TARGET="12.0"

# iOS targets
IOS_TARGETS=(
  "aarch64-apple-ios"
)
IOS_SIM_TARGETS=(
  "aarch64-apple-ios-sim"
)

RUST_LIB_NAME="libffi_mobile"
FRAMEWORK_NAME="ffi_mobile"

# ─── Helpers ─────────────────────────────────────────────────────────────────

log() { echo "==> $*"; }
err() { echo "ERROR: $*" >&2; exit 1; }

check_deps() {
  command -v rustup >/dev/null || err "rustup not found"
  command -v cargo  >/dev/null || err "cargo not found"
  command -v flutter >/dev/null || err "flutter not found"
  command -v xcodebuild >/dev/null || err "xcodebuild not found (install Xcode)"
}

ensure_rust_targets() {
  log "Ensuring Rust iOS targets are installed..."
  for target in "${IOS_TARGETS[@]}" "${IOS_SIM_TARGETS[@]}"; do
    rustup target add "$target" 2>/dev/null || true
  done
}

# ─── Step 1: Build Rust static library for iOS ───────────────────────────────

build_rust() {
  log "Building Rust FFI library for iOS..."

  local rust_mode="release"
  local cargo_flag="--release"
  if [[ "$MODE" == "debug" ]]; then
    rust_mode="debug"
    cargo_flag=""
  fi

  cd "$RUST_CRATE"

  # Build for physical device (arm64)
  for target in "${IOS_TARGETS[@]}"; do
    log "  Compiling for $target..."
    cargo build $cargo_flag --target "$target" --lib
  done

  # Build for simulator (arm64 sim)
  for target in "${IOS_SIM_TARGETS[@]}"; do
    log "  Compiling for $target (simulator)..."
    cargo build $cargo_flag --target "$target" --lib
  done

  local target_dir="$PROJECT_ROOT/target"
  local output_dir="$MOBILE_DIR/ios/Frameworks"
  mkdir -p "$output_dir"

  # Create .framework bundles from dylibs for flutter_rust_bridge
  local device_dylib="$target_dir/aarch64-apple-ios/$rust_mode/${RUST_LIB_NAME}.dylib"
  local sim_dylib="$target_dir/aarch64-apple-ios-sim/$rust_mode/${RUST_LIB_NAME}.dylib"

  if [[ ! -f "$device_dylib" ]]; then
    err "Device dylib not found: $device_dylib"
  fi

  # Build device framework
  local device_fw="$target_dir/aarch64-apple-ios/$rust_mode/${FRAMEWORK_NAME}.framework"
  rm -rf "$device_fw"
  mkdir -p "$device_fw"
  cp "$device_dylib" "$device_fw/$FRAMEWORK_NAME"
  install_name_tool -id "@rpath/${FRAMEWORK_NAME}.framework/$FRAMEWORK_NAME" "$device_fw/$FRAMEWORK_NAME"
  cat > "$device_fw/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key>
  <string>${FRAMEWORK_NAME}</string>
  <key>CFBundleIdentifier</key>
  <string>com.cowallet.ffi-mobile</string>
  <key>CFBundleName</key>
  <string>${FRAMEWORK_NAME}</string>
  <key>CFBundleVersion</key>
  <string>1.0</string>
  <key>CFBundlePackageType</key>
  <string>FMWK</string>
  <key>MinimumOSVersion</key>
  <string>12.0</string>
</dict>
</plist>
PLIST

  # Build simulator framework
  local sim_fw="$target_dir/aarch64-apple-ios-sim/$rust_mode/${FRAMEWORK_NAME}.framework"
  rm -rf "$sim_fw"
  mkdir -p "$sim_fw"
  cp "$sim_dylib" "$sim_fw/$FRAMEWORK_NAME"
  install_name_tool -id "@rpath/${FRAMEWORK_NAME}.framework/$FRAMEWORK_NAME" "$sim_fw/$FRAMEWORK_NAME"
  cp "$device_fw/Info.plist" "$sim_fw/Info.plist"

  # Create XCFramework from dynamic frameworks
  local xcframework="$output_dir/${FRAMEWORK_NAME}.xcframework"
  rm -rf "$xcframework"

  log "  Creating XCFramework..."
  xcodebuild -create-xcframework \
    -framework "$device_fw" \
    -framework "$sim_fw" \
    -output "$xcframework"

  # Clean up old static xcframework if present
  rm -rf "$output_dir/FfiMobile.xcframework"

  log "  Rust build done: $xcframework"
  cd "$PROJECT_ROOT"
}

# ─── Step 2: Generate Flutter-Rust Bridge bindings ───────────────────────────

generate_bindings() {
  log "Generating flutter_rust_bridge bindings..."
  cd "$MOBILE_DIR"
  flutter_rust_bridge_codegen generate 2>&1 | tail -5
  cd "$PROJECT_ROOT"
}

# ─── Step 3: Build Flutter iOS app ──────────────────────────────────────────

build_flutter() {
  log "Building Flutter iOS app (mode: $MODE)..."
  cd "$MOBILE_DIR"

  flutter pub get

  local flutter_cmd="flutter build ios"
  case "$MODE" in
    debug)   flutter_cmd="flutter build ios --debug" ;;
    profile) flutter_cmd="flutter build ios --profile" ;;
    release) flutter_cmd="flutter build ios --release" ;;
    *)       err "Unknown mode: $MODE (use debug|release|profile)" ;;
  esac

  $flutter_cmd $EXTRA_ARGS

  cd "$PROJECT_ROOT"
}

# ─── Main ────────────────────────────────────────────────────────────────────

main() {
  log "CoWallet iOS Build (mode=$MODE)"
  log "─────────────────────────────────────────"

  check_deps
  ensure_rust_targets
  build_rust
  generate_bindings
  build_flutter

  log "─────────────────────────────────────────"
  log "Build complete!"
  log "Output: $MOBILE_DIR/build/ios/iphoneos/Runner.app"
}

main
