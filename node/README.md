# keychain-wallet — Node.js

**English** | [中文](README_ZH.md)

A local macOS key provider backed by the signed `keychain-wallet.app`. The napi-rs bridge returns only the selected public/private key pair after macOS authentication. Supports secp256k1 and SLIP-0010 Ed25519. Requires macOS 14+, Node.js 22+, and a separately built, correctly signed app.

This is a source preview; an npm release has not yet been verified. For now, build as described below and install with `npm install /absolute/path/keychain-wallet/node`. After publication:

```sh
npm install keychain-wallet
```

The release script produces a native addon for Apple Silicon and Intel; release artifacts still need validation. Consumers of a prebuilt release will not need Rust. Install the signed app separately in `~/Applications` or `/Applications` and create or import a wallet first. Replace `trading` below with its saved name; the SDK does not create wallets.

```ts
import { getKeyPair } from 'keychain-wallet';

const { publicKey, privateKey } = await getKeyPair({
  name: 'trading',
  curve: 'secp256k1',
  index: 0,
});
```

`curve` is required. secp256k1 accepts an optional `index` (default 0) or an explicit EVM `path`, but not both. Ed25519 requires an explicit hardened-only path, such as `{ name: 'trading', curve: 'ed25519', path: "m/44'/501'/0'/0'" }`, and does not accept `index`. Use the original wallet's exact derivation path.

Both result fields are `0x` hex strings. secp256k1 returns a 32-byte private scalar and 33-byte compressed public key; Ed25519 returns a 32-byte child seed and 32-byte public key. Never log or persist private keys. Your application manages its own signing library.

The bridge finds `keychain-wallet.app` in `~/Applications` or `/Applications`. In the source checkout, it can also find `build/keychain-wallet.app`. Otherwise pass an absolute executable path as `helperPath`. The app bundle is not included in this npm package; the app and addon must be upgraded together. Use ESM, or `await import('keychain-wallet')` from CommonJS.

Results and failures are cached in the current Node process. Calls must originate from the main thread; different processes authenticate separately. Deleting a wallet or locking the screen does not revoke keys already returned to a process.

## Run the examples

From the repository's `node/` directory, using a wallet already saved as `trading`:

```sh
node --import tsx examples/ethers-v5.ts trading 2
node --import tsx examples/ethers-v6.ts trading 2
node --import tsx examples/viem.ts trading 2
node --import tsx examples/ed25519.ts trading "m/44'/501'/2'/0'"
```

The first three examples print only the Ethereum address; the Ed25519 example prints only the hex public key. `index: 2` selects `m/44'/60'/0'/0/2`. Callers choose the Ed25519 path. Do not pass an Ed25519 seed to ethers `Wallet` to derive the corresponding Solana account.

Node.js 22+ is the supported target. Bun's Node-API support may run these examples, but this project has not completed Bun compatibility testing; a successful individual call is not a full compatibility guarantee.

## Troubleshooting

| Error | What to check |
| --- | --- |
| `NOT_FOUND` | The wallet name must match the app; examples never create `trading` or `demo` |
| `KEYCHAIN_WALLET_APP_NOT_FOUND` | Install the complete app or set `helperPath`; npm does not include the app |
| `KEYCHAIN_WALLET_BRIDGE_NOT_BUILT` | Check addon build and CPU architecture; `cause` contains the original load error |
| `SIGNING_CONFIGURATION` | Check certificate, profile, bundle ID, and private access group using the signing guide |
| `Invalid wallet selection` | curve is required; path and index are exclusive; Ed25519 requires a hardened-only path |
| `CANCELLED` / `AUTHENTICATION_FAILED` | Complete local authentication and retry in a new process |

Failures are cached in the current process. Restart after fixing the cause. Never attach private keys, recovery phrases, or passphrases to an error report.

## Development and publishing

From the repository root:

```sh
pnpm --dir node install --frozen-lockfile --ignore-scripts
pnpm --dir node run build
pnpm --dir node run check
pnpm --dir node test
```

`build` targets the current Mac architecture. The build script lives at `scripts/build-bridge.sh` in the repository root. pnpm manages development dependencies; npm publishes the package. Consumers can use their own package manager.

Before publishing, install both Rust targets and pack from `node/`:

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
cd node
npm pack
```

`prepack` rebuilds both architectures, merges and checks the native addon, then generates JavaScript and type declarations. A failure stops packaging. The package includes only these three artifacts, documentation, and package metadata; no Rust sources, build scripts, or signing material.

Install the generated `keychain-wallet-0.1.0.tgz` in a separate project and verify loading and the public API. After confirming that your publishing account owns the package name, publish that same inspected file with `npm publish ./keychain-wallet-0.1.0.tgz`. Update the version in `package.json` before each release. Build the signed app separately from the repository root.

## License

[MIT](LICENSE)
