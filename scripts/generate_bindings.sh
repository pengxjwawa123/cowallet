#!/usr/bin/env bash
# Generate Dart bindings from Rust FFI using flutter_rust_bridge

set -e

echo "🔧 Generating flutter_rust_bridge bindings..."

# Change to mobile directory
cd "$(dirname "$0")/mobile"

# Run code generation
flutter_rust_bridge_codegen generate \
    --rust-root ../crates/ffi-mobile \
    --dart-root lib/bridge \
    --c-output ios/Runner \
    --dart-decl-output lib/bridge/ffi.dart.generated.dart

echo "✅ Bindings generated successfully!"
