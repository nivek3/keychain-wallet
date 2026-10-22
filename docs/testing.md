# Testing

Run from the repository root:

```sh
pnpm --dir node install --frozen-lockfile --ignore-scripts
bash scripts/check-native.sh
pnpm --dir node run build
pnpm --dir node run check
pnpm --dir node test
```

Rust tests cover derivation vectors, backup confirmation, protected Keychain queries, secure input, clipboard restrictions, the public catalog, deletion plans, and helper transport. Python tests validate packaging with synthetic public profile metadata and exercise universal-addon packaging with stub build tools. Node tests load the native addon and use public fixtures for both curves, key correspondence, authentication serialization, process isolation, and failure handling.

Ed25519 tests check the Solana backend against official SLIP-0010 vectors and public mnemonic results captured before the backend replacement, including account paths, a passphrase, and the maximum hardened index. They also check secret and chain-code zeroization.

## Interactive checks

Use an unfunded test wallet and complete authentication locally. Check creation, backup confirmation, import, key reads, cancellation, Touch ID or password fallback, and profile renewal. Do not send recovery phrases or private keys to chat. Automated tests do not delete real wallets or execute blockchain transactions.

## npm release artifact

Follow [the Node release steps](../node/README.md#development-and-publishing) to build a universal tarball. Install the tarball in a clean Node project; do not use a source-directory symlink for this check. Confirm that importing the package works and an invalid wallet selection is rejected before helper discovery. Check the tarball file list for only package metadata, READMEs, `dist/index.js`, `dist/index.d.ts`, and `dist/keychain-wallet.node`. Run the public-fixture integration tests on both Apple Silicon and Intel before claiming support is verified on both.

Interactive clipboard regression: copy a test phrase, return to the wallet list, then open Sync from Keychain and leave authentication pending. The app's clipboard copy must already be cleared. New unrelated clipboard contents must remain untouched.
