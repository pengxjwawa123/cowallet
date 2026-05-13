#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
JNILIBS_DIR="$PROJECT_ROOT/mobile/android/app/src/main/jniLibs"

NDK_VERSION="${ANDROID_NDK_VERSION:-28.2.13676358}"
ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$HOME/Library/Android/sdk/ndk/$NDK_VERSION}"

if [ ! -d "$ANDROID_NDK_HOME" ]; then
  echo "Error: NDK not found at $ANDROID_NDK_HOME"
  echo "Set ANDROID_NDK_HOME or ANDROID_NDK_VERSION env var"
  exit 1
fi

HOST_TAG="darwin-x86_64"
if [ "$(uname -m)" = "arm64" ] && [ -d "$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/darwin-arm64" ]; then
  HOST_TAG="darwin-arm64"
fi

TOOLCHAIN="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/$HOST_TAG"
API_LEVEL="${ANDROID_API_LEVEL:-24}"

build_target() {
  local rust_target=$1
  local jni_dir=$2
  local cc_prefix=$3

  echo "Building for $rust_target..."

  export CC_${rust_target//-/_}="$TOOLCHAIN/bin/${cc_prefix}${API_LEVEL}-clang"
  export AR_${rust_target//-/_}="$TOOLCHAIN/bin/llvm-ar"
  export CARGO_TARGET_$(echo "$rust_target" | tr '[:lower:]-' '[:upper:]_')_LINKER="$TOOLCHAIN/bin/${cc_prefix}${API_LEVEL}-clang"

  cargo build --release --target "$rust_target" -p ffi-mobile --manifest-path "$PROJECT_ROOT/Cargo.toml"

  mkdir -p "$JNILIBS_DIR/$jni_dir"
  cp "$PROJECT_ROOT/target/$rust_target/release/libffi_mobile.so" "$JNILIBS_DIR/$jni_dir/libffi_mobile.so"

  echo "  -> $JNILIBS_DIR/$jni_dir/libffi_mobile.so"
}

echo "=== CoWallet Android Native Build ==="
echo "NDK: $ANDROID_NDK_HOME"
echo "API Level: $API_LEVEL"
echo ""

# arm64-v8a (primary, modern devices)
build_target "aarch64-linux-android" "arm64-v8a" "aarch64-linux-android"

# armeabi-v7a (older 32-bit ARM devices)
build_target "armv7-linux-androideabi" "armeabi-v7a" "armv7a-linux-androideabi"

# x86_64 (emulator)
build_target "x86_64-linux-android" "x86_64" "x86_64-linux-android"

# x86 (older emulator)
build_target "i686-linux-android" "x86" "i686-linux-android"

echo ""
echo "Done. Run 'cd mobile && flutter run' to test."
