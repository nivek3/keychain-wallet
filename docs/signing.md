# macOS signing

The repository builds the Rust app with Cargo and packages it using `scripts/build-native.sh`. The script requires a code-signing identity in your login Keychain and a valid macOS provisioning profile authorizing the bundle identifier and private Keychain group.

## Personal Team

A free Personal Team manages its App IDs, certificates, devices, and profiles through Xcode. Provisioning expires periodically. Xcode is needed for that account workflow, while this repository's app build uses Cargo and command-line signing. See [Apple's Personal Team documentation](https://developer.apple.com/help/account/basics/about-your-developer-account).

To prepare signing assets:

1. Sign in to your Apple Account in Xcode Settings.
2. Create a temporary macOS App project outside this repository. Set the desired bundle ID, select your Personal Team, enable automatic signing, and configure Keychain Sharing with the app's private access group.
3. Build for My Mac. Locate the signed app through Products → Show in Finder. Its `Contents/embedded.provisionprofile` can be used while valid, with the matching Apple Development signing identity.

If signing assets already exist, use them directly. Do not export certificate private keys or store profiles, passwords, or wallet secrets in Git.

## Build

From the repository root:

```sh
bash scripts/build-native.sh \
  'Apple Development: Your Name (TEAMID)' \
  '/absolute/path/App.provisionprofile' \
  'com.example.keychain-wallet'
```

The script reads the public profile metadata, validates its expiry and authorization, builds the Rust executable, embeds the profile, signs with Hardened Runtime, and verifies the signature. The profile's App Identifier Prefix determines the private access group; it is not assumed to equal Team ID. The output is `build/keychain-wallet.app`. An existing output is replaced only after the new bundle passes signature verification.

```sh
open build/keychain-wallet.app
build/keychain-wallet.app/Contents/MacOS/keychain-wallet check-signing
```

`check-signing` validates public signing configuration. It does not establish that real Keychain authentication succeeds. Use an unfunded test wallet in a logged-in graphical session to check authentication and cancellation.

For local Node use, the SDK discovers `build/keychain-wallet.app` from this source checkout. For a separate installation, place the complete app in `~/Applications` or `/Applications`, or specify the executable's absolute path as `helperPath`.

## Renewal

Renew Personal Team assets through Xcode, then rerun the script with the current profile and matching identity. Keep the bundle ID and authorized access group stable when retaining saved wallets. Profiles do not back up wallet data; keep an independent offline recovery backup.

Do not disable signing checks, the private Keychain group, Hardened Runtime, or user-presence authentication to bypass errors. Ad hoc signing is insufficient. The script does not notarize or publish the app.

References: [command-line signing](https://developer.apple.com/documentation/xcode/creating-distribution-signed-code-for-the-mac/), [Keychain implementations](https://developer.apple.com/documentation/technotes/tn3137-on-mac-keychains).

## Public distribution

The current script builds for the host Mac and does not notarize the app. A Personal Team development build is not a general-purpose download for other users.

For a downloadable release, the publisher needs Developer ID signing through the Apple Developer Program, the entitlements/profile required by this app, notarization, and a stapled ticket. Publish separately identified Apple Silicon and Intel builds, or a validated universal app. Test Keychain creation, read, cancellation, and deletion on a different Mac before advertising the download. End users of that release do not need Xcode, Rust, or their own developer account.

The npm native addon and the signed app are separate artifacts. A universal addon does not make a host-only app universal. Keep the production bundle ID and Keychain access group stable across updates.

See [Apple Developer ID distribution](https://developer.apple.com/developer-id/) and [notarization](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution).
