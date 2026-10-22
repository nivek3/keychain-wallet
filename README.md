# macOS Keychain Wallet

**English** | [中文](README_ZH.md)

A local key provider for macOS and Node.js. Swift handles secure input, Keychain storage, and system authentication. Rust derives secp256k1 and SLIP-0010 Ed25519 keys. A napi-rs bridge handles communication, authentication serialization, in-memory caching, and key-pair verification. TypeScript exposes a small interface; your application uses its own wallet or signing library.

**Status:** Personal Team signing, real Keychain storage and authenticated reads, both curves, and offline signing have been verified with unfunded test wallets on the development Mac. The October 22 directory migration also passed builds, automated checks, and a packed-package consumer check. See the dated [verification record (Chinese)](docs/verification.md) for the exact scope. No funded transactions or RPC broadcasts were tested.

## Repository layout

```text
keychain-wallet/
├── Cargo.toml / Cargo.lock       # Rust workspace and shared lockfile
├── crates/
│   ├── crypto/                  # Key derivation and C ABI for Swift
│   └── node/                    # napi-rs bridge, IPC, and memory cache
├── macos/
│   ├── KeychainWallet.xcodeproj/
│   ├── Sources/                 # Swift UI and Keychain access
│   ├── include/                 # C header
│   ├── Tests/                   # Swift checks and Python config tests
│   └── …                        # Info.plist, entitlements, xcconfig
├── src/index.ts                 # Node API and native-module loading
├── test/                        # Node integration tests
├── examples/
├── scripts/                     # Build and check entry points
├── docs/                        # Signing guide and verification record
└── package.json
```

`npm run build` produces JavaScript, type declarations, and `dist/keychain-wallet.node`. Cargo normally writes to `target/`; Xcode outputs remain under `build/xcode/`. The Swift static library uses the `crypto-release` profile with `panic = "abort"`; the Node addon uses the `release` profile with unwinding.

## Requirements and signing

- macOS 14 or later, running in a logged-in user's graphical session.
- Xcode, Swift, Rust/Cargo, Node.js 22 or later, npm, and Python 3.
- A valid signing identity and provisioning profile authorizing the app identifier and private Keychain group. Xcode can manage local development signing with a free Personal Team.

Open `macos/KeychainWallet.xcodeproj`, sign in to your Apple Account in Xcode Settings, and select your **Personal Team** under the **KeychainWallet target → Signing & Capabilities**. Keep **Automatically manage signing** enabled. Select **My Mac**, build with **Command-B**, then run with **Command-R**. Follow the [Personal Team setup guide](docs/signing.md) for the full steps and troubleshooting.

The shared Run scheme uses Release without a debugger and opens the English Create / Import flow. Keep the entire `.app` bundle intact. The local Xcode Release build is at:

```text
build/xcode/Build/Products/Release/KeychainWallet.app
```

The committed project leaves the Team unset. Choose your own Team locally. Signing assets and wallet secrets are not committed. Personal Team profiles expire; check the actual profile and rebuild/re-sign when needed. Preserve the bundle ID, private access group, and authorized signing identity to retain access to existing entries, and keep an independent offline recovery backup.

The app uses the **Data Protection Keychain** and an embedded provisioning profile. Ad hoc signing is insufficient. A successful compile or `check-signing` result alone does not prove real Keychain authentication works. SSH, headless sessions, daemons, and CI tests cannot establish that the interactive flow works on your Mac. Touch ID is optional; verify password fallback on the actual machine.

## Create or import a wallet

Open the signed app and choose one of its two entry points:

1. **Create a new wallet:** choose a curve and local name. secp256k1 defaults to `m/44'/60'/0'/0/0`; Ed25519 requires an explicit path with every level hardened (ending in `'`). Choose a BIP-39 passphrase or explicitly select **I use no BIP-39 passphrase**. Back up the generated 12-word English recovery phrase and re-enter it in full before saving.
2. **Import an existing wallet:** choose a curve, name, and path. For secp256k1, provide an independently known Ethereum address; for Ed25519, provide the independently known 32-byte public key as `0x` hex, not a base58 chain address. Enter the recovery phrase and optional passphrase manually. A mismatch prevents saving.

The success page shows the name, curve, path, and public key; secp256k1 also shows an address. Cancelling does not create an entry. An existing name returns `ALREADY_EXISTS` and is never overwritten. To replace a wallet, import under a new name and verify it before separately deciding whether to delete the old entry.

A BIP-39 passphrase is **not your Mac login password or a wallet application's unlock password**. Leaving it empty is supported through the explicit checkbox. Nonempty passphrases require confirmation; spaces are preserved and BIP-39 NFKD normalization applies. A different passphrase derives a different wallet. Here it is stored with the recovery phrase in the same authenticated entry, so it is not an independent second authentication factor. Only English BIP-39 phrases are accepted.

The creation backup page offers **Copy recovery phrase**. Only the new-wallet confirmation field permits **Paste recovery phrase / Cmd+V**. Import and passphrase fields disable copying, cutting, and pasting. While the app remains running, it attempts to clear its copied phrase after 60 seconds without clearing newer clipboard content. Clipboard managers and other apps may retain copies; clearing does not revoke those copies. Keep an offline backup. There is no routine recovery-phrase export command.

Start with an unfunded test wallet. Never send a real recovery phrase, passphrase, or private key to chat or place it in shell arguments, environment variables, or configuration files.

## Use from Node.js / TypeScript

Build this repository, then install the local package from your application:

```sh
# In this repository:
npm ci --ignore-scripts
npm run build

# In your Node application; replace with the repository's actual path:
npm install /absolute/path/keychain-wallet
```

The package is ESM. CommonJS callers can use `await import('macos-keychain-wallet')`.

```ts
import { getKeyPair } from 'macos-keychain-wallet';

const { publicKey, privateKey } = await getKeyPair({
  name: 'demo',
  curve: 'secp256k1',
  index: 0, // Optional: 0 selects the first account key.
});
```

`curve` is required. For Ed25519, pass a complete SLIP-0010 path:

```ts
const { publicKey, privateKey } = await getKeyPair({
  name: 'demo',
  curve: 'ed25519',
  path: "m/44'/501'/0'/0'",
});
```

Use the exact path convention of the wallet you are restoring. A curve alone does not define a chain or wallet's path, and this implementation does not support every Ed25519 derivation scheme. The same saved phrase and passphrase can derive either curve without being imported again; their master keys use each standard's domain separation.

The result contains only two `0x` hex strings. **Do not log the result or the private key.**

| Curve | `privateKey` | `publicKey` |
| --- | --- | --- |
| `secp256k1` | 32-byte private scalar | 65-byte uncompressed public key, starting with `0x04` |
| `ed25519` | 32-byte seed for the selected child key | 32-byte public key |

An Ed25519 private key here is not the BIP-39 root seed, an expanded scalar, or a 64-byte SDK key pair. Use a downstream SDK import method that accepts a 32-byte seed.

| Selection | Rules |
| --- | --- |
| `name` | Required local wallet name. 1–64 ASCII characters; start with a letter or digit, followed by letters, digits, `.`, `_`, or `-`. |
| `curve` | Required: `secp256k1` or `ed25519`. No automatic curve detection. |
| secp256k1 `index` | Optional integer from 0 through 2³¹−1, default 0. Derives `m/44'/60'/0'/0/index`. |
| secp256k1 `path` | Optional full `m/44'/60'/account'/change/index` path; `change` is 0 or 1 and numeric components are below 2³¹. Cannot be combined with `index`. |
| Ed25519 `path` | Required; at least one level after `m`, every level hardened, each value below 2³¹. `index` is rejected. |
| `helperPath` | Optional absolute path to the signed app's executable for a nonstandard installation. |

Paths are limited to 128 bytes. `getKeyPair` does not require `address` or `expectedAddress`. The GUI import flow still verifies an independently known address or public key.

The SDK discovers the helper in this order:

1. `~/Applications/KeychainWallet.app`
2. `/Applications/KeychainWallet.app`
3. This package's `build/xcode/Build/Products/Release/KeychainWallet.app`

It does not search the working directory or `PATH`. An explicitly selected broken helper does not silently fall back to another app. A packaged installation needs an installed app or an explicit `helperPath`, since build outputs are not part of the npm package. Rebuild the native addon when changing Mac architecture; there is no automatic binary download or postinstall build.

Your application owns address formatting, signing, and RPC access. For example, after obtaining `privateKey`:

```ts
// viem
import { privateKeyToAccount } from 'viem/accounts';
const account = privateKeyToAccount(privateKey);

// ethers v6 (v5 also provides Wallet)
import { Wallet } from 'ethers';
const signer = new Wallet(privateKey);
```

See the [viem](examples/viem.ts), [ethers v5](examples/ethers-v5.ts), and [ethers v6](examples/ethers-v6.ts) examples. These wallet libraries are chosen by the application, not wrapped by this SDK.

## Authentication and caching

Keychain entries use the signed app's private access group, `.userPresence`, and `WhenUnlockedThisDeviceOnly`, with iCloud synchronization disabled. The access-control requirement is attached to the entry itself.

On the first request, the Rust bridge starts a short-lived native helper. The helper creates a fresh `LAContext` with authentication reuse disabled; `SecItemCopyMatching` triggers system authentication. macOS chooses Touch ID or device-owner password. The tool does not collect your Mac password.

Only the selected 32-byte private key goes through fd 3, a private pipe/socketpair created by Node. stdout carries public metadata. The recovery phrase, BIP-39 root seed, and passphrase are not sent to Node. The bridge checks the returned public/private key correspondence using k256 or ed25519-dalek. The app and SDK must be upgraded together; older helpers without the required curve/public-key metadata are rejected. Public requests are limited to 2 KiB, and a 120-second authentication timeout terminates the helper.

Within one loaded Rust module instance, requests for the same helper, name, curve, and resolved path share the result. Authentication is serialized; concurrent matching requests share one read. Both success and failure remain cached for the process lifetime. Fix the cause and restart the business process after a cached failure. Different processes and different derived accounts authenticate separately. Loading another copy of the native module is outside this deduplication guarantee.

Call from Node's main thread; worker-thread calls are rejected. Authentication waits use Rust's blocking task pool rather than Node's libuv file-operation pool. There is no background service, network listener, shared unlock state, or disk cache. Locking the screen does not revoke a private key already returned to the application.

Errors use fixed messages such as `CANCELLED`, `AUTHENTICATION_FAILED`, `NOT_FOUND`, `SIGNING_CONFIGURATION`, `INVALID_HELPER_RESPONSE`, and `HELPER_START_FAILED`. Missing app/module errors are `KEYCHAIN_WALLET_APP_NOT_FOUND` and `KEYCHAIN_WALLET_BRIDGE_NOT_BUILT`. Cancellation and incorrect passwords can map to the same authentication error. Failures do not fall back to `.env` or expose raw helper errors.

## Build and offline checks

Run these from the repository root:

```sh
npm ci --ignore-scripts
npm run build
npm run check
npm test
python3 -m unittest discover -s macos/Tests -p 'test_*.py'
bash scripts/check-native.sh
```

`npm ci` uses `package-lock.json`; Rust scripts use the root `Cargo.lock` with `--locked`. Installing dependencies may contact npm and crates.io. There is no JavaScript cryptography fallback when Rust is unavailable.

Use Xcode for automatically managed signing. If you already have a suitable certificate and profile, the manual build alternative is:

```sh
# Public signing configuration only; replace all three values.
bash scripts/build-native.sh \
  'Apple Development: Your Name (TEAMID)' \
  '/absolute/path/Wallet.provisionprofile' \
  'com.yourname.KeychainWallet'
```

This script creates a new app bundle, embeds the profile, enables Hardened Runtime, and signs it. It does not register an App ID, download certificates, export private keys, notarize, or publish the app. The profile's App Identifier Prefix is not assumed to equal its Team ID. Only the app identifier, Team identifier, and one private Keychain group are signed; debug entitlements are not injected. Runtime checks reject invalid/ad hoc signing, a missing profile, a wrong access group, missing Hardened Runtime, and unsafe debugging entitlements. The operating system makes the final entitlement-authorization decision.

The October 22 checks passed: Node 10/10, Rust 13/13, Python 4/4, Swift/Rust public-vector checks, clipboard checks, Xcode Release signing, strict signature verification, and installing a packed package into a separate Node consumer. That refactor did not repeat GUI creation/import, real Keychain authentication, or the manual-profile build branch. Earlier dated verification covers the real UI and authentication checks.

The earlier npm audit reported 14 low-severity findings in the ethers v5 development dependency chain; this documentation change does not constitute a fresh dependency or security audit. No npm production dependencies does not mean the Rust dependencies or application wallet libraries have been audited.

## Public CLI management

The CLI uses the executable inside the signed bundle. The following names, paths, and address are public test data:

```sh
WALLET_HELPER='/absolute/path/KeychainWallet.app/Contents/MacOS/KeychainWallet'

# Opens secure local import fields for an unfunded public Hardhat account.
"$WALLET_HELPER" import demo "m/44'/60'/0'/0/0" \
  '0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266'

# Lists public names without reading secrets; fails if attributes require interaction.
"$WALLET_HELPER" list

# Authenticates, derives, and outputs only an address matching this expected value.
"$WALLET_HELPER" address demo "m/44'/60'/0'/0/0" \
  '0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266'

# Only when intentionally deleting an entry: local confirmation and authentication.
"$WALLET_HELPER" delete demo
```

For this public test account only, manually enter `test test test test test test test test test test test junk` in the import window and select **I use no BIP-39 passphrase**. **Never fund this public account.** For a real import, independently obtain the expected address/public key from the existing wallet or a trusted offline backup. Do not fill it from this tool's own derived output.

The legacy `import` / `address` CLI supports only secp256k1 and Ethereum addresses; use the GUI or Node SDK for both curves. Address comparison is case-insensitive by bytes; it does not claim to validate EIP-55 input checksums. Secret input is never read from arguments, files, environment variables, or the clipboard during import.

## Security boundaries and remaining checks

This tool reduces long-lived secret exposure in `.env` files, configuration backups, and source trees. It protects storage and requires system authentication; the selected private key still enters Node memory and any downstream wallet objects.

It does not protect against malicious application code or dependencies, attackers who can read process memory, a replaced trusted helper, system compromise, keylogging, or screen monitoring. The authentication dialog identifies the helper and requested account, not a trusted Node caller. Other trusted programs authorized for the same signing/access group are inside the security boundary.

The native app disables core dumps, Rust zeroizes seed/private-key buffers, and Swift clears explicit byte buffers where possible. Complete erasure of Swift/Foundation strings, library temporaries, JavaScript strings, GC memory, swap, or crash/diagnostic dumps is not guaranteed. Keep secret-bearing diagnostics and debuggers disabled. The implementation does not perform EVM signing inside the Secure Enclave; your Node wallet library signs with the returned key.

Keychain migration does not make an already leaked key safe. This tool does not move funds, revoke approvals, delete old secret files, or broadcast transactions. `ThisDeviceOnly` storage with synchronization disabled is not a cross-device recovery mechanism. Keep an independent offline backup.

Real authentication results are specific to the tested Mac and signing configuration. Touch ID/password fallback, cancellation/incorrect passwords, reads after screen locking, and signing renewal still need separate validation as recorded in the [verification record (Chinese)](docs/verification.md). No real-funds transactions have been tested.

## References

- Apple: [Keychain implementations and provisioning](https://developer.apple.com/documentation/technotes/tn3137-on-mac-keychains), [restricted-entitlement bundles](https://developer.apple.com/documentation/xcode/signing-a-daemon-with-a-restricted-entitlement), [Keychain access controls](https://developer.apple.com/documentation/security/restricting-keychain-item-accessibility), [authentication contexts](https://developer.apple.com/documentation/localauthentication/accessing-keychain-items-with-face-id-or-touch-id), [Mac distribution signing](https://developer.apple.com/documentation/xcode/creating-distribution-signed-code-for-the-mac/).
- Cryptography: [rust-bip39](https://github.com/rust-bitcoin/rust-bip39), [bip32 0.5.3](https://docs.rs/bip32/0.5.3/bip32/), [Keccak](https://docs.rs/sha3/0.10.8/sha3/), [ed25519-dalek-bip32 0.3.0](https://docs.rs/ed25519-dalek-bip32/0.3.0/ed25519_dalek_bip32/), [ed25519-dalek 2.2.0](https://docs.rs/ed25519-dalek/2.2.0/ed25519_dalek/), [SLIP-0010](https://github.com/satoshilabs/slips/blob/master/slip-0010.md), [RFC 8032](https://www.rfc-editor.org/rfc/rfc8032.html).
- Integration: [napi-rs](https://napi.rs/docs/concepts/async-task), [viem](https://viem.sh/docs/accounts/local/privateKeyToAccount), [ethers v5](https://docs.ethers.org/v5/api/signer/#Wallet), [ethers v6](https://docs.ethers.org/v6/api/wallet/#Wallet), [Solana recovery-path example](https://solana.com/developers/cookbook/wallets/restore-from-mnemonic).
- UI reference: MetaMask's [creation/import entry points](https://support.metamask.io/hc/en-us/articles/360015489531) and [recovery-phrase import flow](https://support.metamask.io/start/use-an-existing-wallet).
