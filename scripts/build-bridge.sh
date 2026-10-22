#!/bin/bash
set -euo pipefail
umask 077
root=$(cd "$(dirname "$0")/.." && pwd)
if [[ $(uname -s) != Darwin ]]; then
  echo 'macOS is required' >&2
  exit 1
fi
export CARGO_TARGET_DIR="$root/target"
cargo build --release --locked --manifest-path "$root/Cargo.toml" -p wallet-bridge
mkdir -p "$root/dist"
cp "$root/target/release/libwallet_bridge.dylib" "$root/dist/keychain-wallet.node"
