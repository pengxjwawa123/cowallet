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

# iOS targets
IOS_TARGETS=(
  "aarch64-apple-ios"
)
IOS_SIM_TARGETS=(
  "aarch64-apple-ios-sim"
)

RUST_LIB_NAME="libffi_mobile"

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

  local target_dir="$RUST_CRATE/target"
  local output_dir="$MOBILE_DIR/ios/Frameworks"
  mkdir -p "$output_dir"

  # Create XCFramework from static libs
  local device_lib="$target_dir/aarch64-apple-ios/$rust_mode/$RUST_LIB_NAME.a"
  local sim_lib="$target_dir/aarch64-apple-ios-sim/$rust_mode/$RUST_LIB_NAME.a"

  if [[ ! -f "$device_lib" ]]; then
    err "Device library not found: $device_lib"
  fi

  local xcframework="$output_dir/FfiMobile.xcframework"
  rm -rf "$xcframework"

  log "  Creating XCFramework..."
  xcodebuild -create-xcframework \
    -library "$device_lib" \
    -library "$sim_lib" \
    -output "$xcframework"

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
