#!/bin/bash
set -euo pipefail
umask 077

if [[ $# != 3 ]]; then
  echo 'Usage: bash scripts/build-native.sh <signing identity> <profile path> <bundle id>' >&2
  exit 1
fi
identity=$1
profile=$2
bundle_id=$3
if [[ "$identity" == '-' || ! -f "$profile" ]]; then
  echo 'A real signing identity and provisioning profile are required.' >&2
  exit 1
fi
root=$(cd "$(dirname "$0")/.." && pwd)
mkdir -p "$root/build"
app=$(mktemp -d "$root/build/KeychainWallet.XXXXXXXX")/KeychainWallet.app
mkdir -p "$app/Contents/MacOS"
# The decoded profile contains signing metadata, never wallet secrets.
/usr/bin/security cms -D -i "$profile" -o "${app%/*}/profile.plist"
/usr/bin/python3 "$root/scripts/prepare-bundle.py" "${app%/*}/profile.plist" "$bundle_id" "$app"
cp "$profile" "$app/Contents/embedded.provisionprofile"

export CARGO_TARGET_DIR="$root/target"
cargo build --profile crypto-release --locked --manifest-path "$root/Cargo.toml" -p wallet-crypto
xcrun swiftc -O -swift-version 5 -target "$(uname -m)-apple-macos14.0" \
  -module-cache-path "$root/build/swift-module-cache" \
  -import-objc-header "$root/macos/include/WalletCrypto.h" \
  "$root/macos/Sources/WalletCore.swift" "$root/macos/Sources/main.swift" "$root/target/crypto-release/libwallet_crypto.a" \
  -liconv -lresolv \
  -framework AppKit -framework Foundation -framework Security -framework LocalAuthentication \
  -o "$app/Contents/MacOS/KeychainWallet"

/usr/bin/codesign --force --sign "$identity" --options runtime --timestamp \
  --entitlements "${app%/*}/entitlements.plist" "$app"
/usr/bin/codesign --verify --strict --verbose=2 "$app"
echo "Built app: $app"
echo 'Do not import a real wallet until the documented test-wallet checks pass.'
