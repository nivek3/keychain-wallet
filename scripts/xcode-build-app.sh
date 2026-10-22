#!/bin/bash
set -euo pipefail
umask 077
: "${SRCROOT:?Run this build phase from Xcode}"
: "${TARGET_BUILD_DIR:?Missing Xcode build output directory}"
: "${EXECUTABLE_PATH:?Missing app executable path}"
: "${NATIVE_ARCH_ACTUAL:?Missing native Mac architecture}"
case "$NATIVE_ARCH_ACTUAL" in
  arm64) rust_target=aarch64-apple-darwin ;;
  x86_64) rust_target=x86_64-apple-darwin ;;
  *) echo 'Unsupported native Mac architecture.' >&2; exit 1 ;;
esac
export PATH="$HOME/.cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin"
if ! command -v cargo >/dev/null 2>&1; then
  echo 'Rust/Cargo is required. Install it locally before building this target.' >&2
  exit 1
fi
root=$(cd "$SRCROOT/.." && pwd)
export CARGO_TARGET_DIR="$root/build/xcode-rust"
export MACOSX_DEPLOYMENT_TARGET=14.0
cargo build --release --locked --target "$rust_target" --manifest-path "$root/Cargo.toml" -p wallet-app
mkdir -p "$(dirname "$TARGET_BUILD_DIR/$EXECUTABLE_PATH")"
cp "$CARGO_TARGET_DIR/$rust_target/release/KeychainWallet" "$TARGET_BUILD_DIR/$EXECUTABLE_PATH"
