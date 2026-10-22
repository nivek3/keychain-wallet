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
staging=$(mktemp -d "$root/build/.keychain-wallet.XXXXXXXX")
app="$staging/keychain-wallet.app"
mkdir -p "$app/Contents/MacOS"
# The decoded profile contains signing metadata, never wallet secrets.
/usr/bin/security cms -D -i "$profile" -o "${app%/*}/profile.plist"
/usr/bin/python3 "$root/scripts/prepare-bundle.py" "${app%/*}/profile.plist" "$bundle_id" "$app"
cp "$profile" "$app/Contents/embedded.provisionprofile"

export CARGO_TARGET_DIR="$root/target"
export MACOSX_DEPLOYMENT_TARGET=14.0
cargo build --release --locked --manifest-path "$root/Cargo.toml" -p wallet-app
cp "$root/target/release/keychain-wallet" "$app/Contents/MacOS/keychain-wallet"

/usr/bin/codesign --force --sign "$identity" --options runtime --timestamp \
  --entitlements "${app%/*}/entitlements.plist" "$app"
/usr/bin/codesign --verify --strict --verbose=2 "$app"
output="$root/build/keychain-wallet.app"
if [[ -e "$output" ]]; then
  mv "$output" "$staging/previous.app"
fi
if ! mv "$app" "$output"; then
  if [[ -e "$staging/previous.app" ]]; then
    mv "$staging/previous.app" "$output"
  fi
  exit 1
fi
rm -rf "$staging"
echo "Built app: $output"
