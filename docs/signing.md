# Local setup with a free Personal Team

This project supports the free development-signing route. Apple lists macOS Keychain Sharing as available to free Apple Developers. This is a capability statement, not proof that this app has completed real Keychain authentication with your account. Keep using a public, unfunded test wallet until those checks pass.

## What is already prepared

- Open `macos/KeychainWallet.xcodeproj`; no temporary app or separate profile-generation project is needed.
- The target builds the actual Swift implementation and the existing Rust crypto library.
- Automatic signing and the app's private Keychain group are configured. The Team is intentionally unset so you choose your own Personal Team.
- The default Bundle ID is `dev.phcxc.KeychainWallet.a15e8e`. You can change it in the target's General tab before importing any wallet; the app identifier and access group follow that value.
- Both configurations enable Hardened Runtime and disable `get-task-allow` injection. The shared Run scheme uses Release and does not attach a debugger.
- Run opens the English Create / Import setup flow. No wallet is saved until you complete that flow. The separate `check-signing` command still checks only public signing metadata.
- No Apple account, Team ID, certificate private key, profile, or wallet secret is committed.

## Your steps in Xcode

1. Open Xcode itself first and review/accept its license agreement if prompted. This machine has Xcode 27.1 installed and has completed the initial signing build. On a new machine, complete any required first-launch setup yourself, then open the project from this repository. If Xcode asks whether to trust this project, review the local Rust build script before trusting it; that script builds the locked Rust dependencies.
2. In Xcode Settings, open the Apple Accounts / Accounts pane and sign in to your Apple Account. Complete password and verification-code prompts locally.
3. Select the blue project icon, then the **KeychainWallet target** and **Signing & Capabilities**. Keep **Automatically manage signing** enabled and select **your name (Personal Team)** for Team. Keychain Sharing is already configured; do not add a second group.
4. Select the **KeychainWallet** scheme and **My Mac**, then press **Command-B**. Xcode can create/manage the development certificate and profile. If a certificate is needed, use your account's **Manage Certificates → + → Apple Development**. Do not export or send its private key.
5. Press **Command-R** with the shared scheme to open **Create a new wallet / Import an existing wallet**. Creation requires backing up and re-entering the new recovery phrase before saving. Import requires an independently known Ethereum address (secp256k1) or 32-byte hex public key (Ed25519). For a separate signing check, run the signed executable with `check-signing`; expected output includes `"status":"SIGNING_CONFIGURATION_OK"`. This check alone does not prove the Keychain ACL or authentication fallback works.
6. Under **Products**, right-click **KeychainWallet.app → Show in Finder** to locate the signed app. Keep the entire app bundle intact. Node automatically finds this package's local Xcode Release build, or an installed app in `~/Applications` / `/Applications`; business code can omit `helperPath`. Command-line management still uses the executable path. A nonstandard installation can set `helperPath` explicitly.

On this machine Xcode is at `/Applications/Xcode.app`. The system command-line selection may still point to Command Line Tools; this does not prevent building inside Xcode. For terminal project inspection/builds use `DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer` on the command, rather than changing the global toolchain setting.

First builds may download public crates from crates.io. Rust is found through `$HOME/.cargo/bin`; its existing toolchain must be installed. This local project builds only the current Mac's architecture.

## If signing fails

Copy the error message, without passwords, tokens, private keys, or wallet words. Common conditions to check are an unset Team, a Bundle ID already taken, unavailable profile authorization, expired signing assets, or an incomplete Apple Account login.

Do not turn off the private Keychain group, user-presence requirement, Hardened Runtime, or the runtime signing checks to get past an error. Do not enable **Debug executable**, `get-task-allow`, or **Disable Library Validation** for wallet use. The runtime intentionally rejects those unsafe configurations.

If `SIGNING_CONFIGURATION` is returned, inspect whether the app contains `Contents/embedded.provisionprofile` and whether the signed entitlements match the app identifier and its one private Keychain group. Generated assets, rather than a successful compile alone, determine whether this route is usable.

## After signing succeeds

Follow the [README](../README.md) to import the public Hardhat test mnemonic through the local secure fields. Verify real macOS authentication, cancellation/failure, address matching, independent business processes, and a no-interaction Keychain read rejection before importing a real wallet. Never fund the public test address.

Personal Team is development signing and has provisioning/renewal limits. Check the actual profile's expiration and re-sign/rebuild as required. Rebuilding the same app should preserve its Bundle ID and access group. Do not delete the wallet entry during renewal; retain an independent offline backup.

## Apple references

- [macOS supported capabilities](https://developer.apple.com/help/account/reference/supported-capabilities-macos/)
- [Personal Team and membership limitations](https://developer.apple.com/help/account/basics/about-your-developer-account)
- [Xcode signing workflow](https://help.apple.com/xcode/mac/current/en.lproj/dev60b6fbbc7.html)
- [Why base debug entitlements are disabled](https://developer.apple.com/documentation/security/resolving-common-notarization-issues)
- [macOS Keychain implementations and profiles](https://developer.apple.com/documentation/technotes/tn3137-on-mac-keychains)
