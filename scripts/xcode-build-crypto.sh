#!/bin/bash
set -euo pipefail
umask 077
: "${SRCROOT:?Run this build phase from Xcode}"
: "${BUILT_PRODUCTS_DIR:?Missing Xcode build output directory}"
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
cargo build --profile crypto-release --locked --target "$rust_target" --manifest-path "$root/Cargo.toml" -p wallet-crypto
mkdir -p "$BUILT_PRODUCTS_DIR"
cp "$CARGO_TARGET_DIR/$rust_target/crypto-release/libwallet_crypto.a" "$BUILT_PRODUCTS_DIR/libwallet_crypto.a"
