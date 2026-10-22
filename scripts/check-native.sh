#!/bin/bash
set -euo pipefail
umask 077
root=$(cd "$(dirname "$0")/.." && pwd)
mkdir -p "$root/build"
export CARGO_TARGET_DIR="$root/target"
cargo test --workspace --locked --manifest-path "$root/Cargo.toml"
cargo build --profile crypto-release --locked --manifest-path "$root/Cargo.toml" -p wallet-crypto
xcrun swiftc -O -swift-version 5 -target "$(uname -m)-apple-macos14.0" \
  -module-cache-path "$root/build/swift-module-cache" \
  -import-objc-header "$root/macos/include/WalletCrypto.h" \
  "$root/macos/Sources/WalletCore.swift" "$root/macos/Tests/Core/main.swift" \
  "$root/target/crypto-release/libwallet_crypto.a" \
  -liconv -lresolv -framework Foundation -o "$root/build/native-selfcheck"
"$root/build/native-selfcheck"

clipboard_check=$(mktemp -d "$root/build/clipboard-check.XXXXXX")
python3 - "$root/macos/Sources/main.swift" "$clipboard_check/Clipboard.swift" <<'PYTHON'
from pathlib import Path
import sys
source = Path(sys.argv[1]).read_text()
start = source.index("final class RecoveryPhraseClipboard:")
end = source.index("func showMessage(", start)
Path(sys.argv[2]).write_text("import AppKit\n" + source[start:end])
PYTHON
xcrun swiftc -O -swift-version 5 -target "$(uname -m)-apple-macos14.0" \
  -module-cache-path "$root/build/swift-module-cache" \
  "$clipboard_check/Clipboard.swift" "$root/macos/Tests/Clipboard/main.swift" \
  -framework AppKit -o "$clipboard_check/selfcheck"
"$clipboard_check/selfcheck"
