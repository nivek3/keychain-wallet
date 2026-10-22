import AppKit
import Darwin
import Foundation
import LocalAuthentication
import Security

func checkStatus(_ status: OSStatus) throws {
    switch status {
    case errSecSuccess: return
    case errSecUserCanceled: throw Failure.cancelled
    case errSecAuthFailed, errSecInteractionNotAllowed: throw Failure.authentication
    case errSecItemNotFound: throw Failure.missing
    case errSecDuplicateItem: throw Failure.duplicate
    case errSecMissingEntitlement: throw Failure.configuration
    default: throw Failure.keychain
    }
}

struct Vault {
    let group: String
    let service = "local.keychain-wallet.mnemonic.v1"

    init() throws {
        let bundle = Bundle.main
        guard let bundleID = bundle.bundleIdentifier,
              let group = bundle.object(forInfoDictionaryKey: "WalletAccessGroup") as? String,
              group.hasSuffix("." + bundleID), !group.contains("*"),
              FileManager.default.fileExists(atPath: bundle.bundleURL.appendingPathComponent("Contents/embedded.provisionprofile").path)
        else { throw Failure.configuration }
        var code: SecCode?
        var staticCode: SecStaticCode?
        var info: CFDictionary?
        guard SecCodeCopySelf([], &code) == errSecSuccess, let code,
              SecCodeCheckValidity(code, [], nil) == errSecSuccess,
              SecCodeCopyStaticCode(code, [], &staticCode) == errSecSuccess, let staticCode,
              SecCodeCopySigningInformation(staticCode, SecCSFlags(rawValue: kSecCSSigningInformation), &info) == errSecSuccess,
              let dict = info as? [String: Any],
              let entitlements = dict[kSecCodeInfoEntitlementsDict as String] as? [String: Any],
              let groups = entitlements["keychain-access-groups"] as? [String], groups == [group],
              entitlements["com.apple.application-identifier"] as? String == group,
              let flags = dict[kSecCodeInfoFlags as String] as? NSNumber,
              flags.uint32Value & SecCodeSignatureFlags.runtime.rawValue != 0,
              flags.uint32Value & SecCodeSignatureFlags.adhoc.rawValue == 0 else { throw Failure.configuration }
        for entitlement in ["com.apple.security.get-task-allow", "get-task-allow",
                            "com.apple.security.cs.disable-library-validation",
                            "com.apple.security.cs.allow-dyld-environment-variables",
                            "com.apple.security.cs.allow-unsigned-executable-memory"] {
            if entitlements[entitlement] as? Bool == true { throw Failure.configuration }
        }
        self.group = group
    }

    func query(_ name: String? = nil) -> [String: Any] {
        var q: [String: Any] = [kSecClass as String: kSecClassGenericPassword,
            kSecUseDataProtectionKeychain as String: true,
            kSecAttrAccessGroup as String: group, kSecAttrService as String: service,
            kSecAttrSynchronizable as String: false]
        if let name { q[kSecAttrAccount as String] = name }
        return q
    }

    func insert(_ name: String, _ wallet: StoredWallet) throws {
        var error: Unmanaged<CFError>?
        guard let acl = SecAccessControlCreateWithFlags(nil, kSecAttrAccessibleWhenUnlockedThisDeviceOnly,
                                                        .userPresence, &error) else { throw Failure.configuration }
        var data = try JSONEncoder().encode(wallet)
        defer { data.resetBytes(in: 0..<data.count) }
        var q = query(name)
        q[kSecAttrAccessControl as String] = acl
        q[kSecAttrLabel as String] = "Keychain Wallet: " + name
        q[kSecValueData as String] = data
        // Deliberately no update-on-duplicate and no file-keychain fallback.
        try checkStatus(SecItemAdd(q as CFDictionary, nil))
    }

    func read(_ name: String, context: LAContext) throws -> StoredWallet {
        var q = query(name)
        q[kSecUseAuthenticationContext as String] = context
        q[kSecReturnData as String] = true
        q[kSecMatchLimit as String] = kSecMatchLimitOne
        var item: CFTypeRef?
        try checkStatus(SecItemCopyMatching(q as CFDictionary, &item))
        guard var data = item as? Data, data.count <= 16384 else { throw Failure.corrupt }
        defer { data.resetBytes(in: 0..<data.count) }
        guard let wallet = try? JSONDecoder().decode(StoredWallet.self, from: data), wallet.version == 1 else {
            throw Failure.corrupt
        }
        return wallet
    }

    func names() throws -> [String] {
        var q = query()
        q[kSecReturnAttributes as String] = true
        q[kSecMatchLimit as String] = kSecMatchLimitAll
        let context = LAContext()
        context.interactionNotAllowed = true
        defer { context.invalidate() }
        q[kSecUseAuthenticationContext as String] = context
        var items: CFTypeRef?
        let status = SecItemCopyMatching(q as CFDictionary, &items)
        if status == errSecItemNotFound { return [] }
        try checkStatus(status)
        guard let rows = items as? [[String: Any]] else { throw Failure.corrupt }
        return rows.compactMap { $0[kSecAttrAccount as String] as? String }.sorted()
    }

    func delete(_ name: String, context: LAContext) throws {
        var q = query(name)
        q[kSecUseAuthenticationContext as String] = context
        try checkStatus(SecItemDelete(q as CFDictionary))
    }
}

// Clipboard access is limited to explicit backup copy and creation-confirmation paste.
final class WalletApplication: NSApplication {
    override func sendAction(_ action: Selector, to target: Any?, from sender: Any?) -> Bool {
        let name = NSStringFromSelector(action)
        if ["paste:", "pasteAsPlainText:"].contains(name),
           let field = keyWindow?.initialFirstResponder as? SecretField,
           field.allowsPaste, field.currentEditor() === keyWindow?.firstResponder {
            field.pasteRecoveryPhrase(nil)
            return true
        }
        if ["paste:", "pasteAsPlainText:", "pasteAsRichText:", "copy:", "cut:"].contains(name) { return false }
        return super.sendAction(action, to: target, from: sender)
    }
}

final class SecretField: NSSecureTextField {
    var allowsPaste = false

    @objc func pasteRecoveryPhrase(_ sender: Any?) {
        guard allowsPaste, let text = NSPasteboard.general.string(forType: .string),
              text.utf8.count <= 4096 else { NSSound.beep(); return }
        if let editor = currentEditor() as? NSTextView {
            editor.insertText(text, replacementRange: editor.selectedRange())
        } else { stringValue = text }
    }

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        if event.modifierFlags.contains(.command) {
            let key = event.charactersIgnoringModifiers?.lowercased() ?? ""
            if key == "v", allowsPaste { pasteRecoveryPhrase(nil); return true }
            if ["v", "c", "x"].contains(key) { return true }
        }
        return super.performKeyEquivalent(with: event)
    }
    override func textDidBeginEditing(_ notification: Notification) {
        super.textDidBeginEditing(notification)
        currentEditor()?.menu = nil
        (currentEditor() as? NSTextView)?.isAutomaticTextCompletionEnabled = false
        (currentEditor() as? NSTextView)?.isAutomaticSpellingCorrectionEnabled = false
    }
}

final class RecoveryPhraseClipboard: NSObject {
    let words: String
    let pasteboard: NSPasteboard
    var clearTimer: Timer?

    init(words: String, pasteboard: NSPasteboard = .general) {
        self.words = words
        self.pasteboard = pasteboard
    }

    @objc func copyPhrase(_ sender: NSButton) {
        pasteboard.clearContents()
        guard pasteboard.setString(words, forType: .string) else {
            sender.title = "Copy failed — try again"
            return
        }
        sender.title = "Copied"
        clearTimer?.invalidate()
        let count = pasteboard.changeCount
        let board = pasteboard
        let timer = Timer(timeInterval: 60, repeats: false) { _ in
            if board.changeCount == count { board.clearContents() }
        }
        clearTimer = timer
        RunLoop.main.add(timer, forMode: .common)
        RunLoop.main.add(timer, forMode: .modalPanel)
    }
}

func showMessage(_ title: String, _ message: String) {
    let alert = NSAlert()
    alert.messageText = title
    alert.informativeText = message
    alert.addButton(withTitle: "OK")
    NSApp.activate(ignoringOtherApps: true)
    alert.runModal()
}

func inputMessage(_ error: Error) -> String {
    switch error as? Failure {
    case .mnemonic: return "Check the English recovery words, their order, and checksum."
    case .address: return "The recovery phrase, BIP-39 passphrase, and path do not match the expected address."
    case .publicKey: return "The recovery phrase, BIP-39 passphrase, and path do not match the expected Ed25519 public key."
    case .duplicate: return "That wallet name already exists. Choose another name; the existing wallet was not changed."
    case .configuration: return "The app's signing configuration is invalid. Rebuild with your Personal Team."
    case .authentication: return "macOS authentication was not completed. Nothing was unlocked."
    default: return "Check the wallet name, curve, derivation path, expected address or public key, and matching passphrase fields."
    }
}

func form(_ fields: [(String, NSTextField)]) -> NSStackView {
    let stack = NSStackView()
    stack.orientation = .vertical
    stack.alignment = .leading
    stack.spacing = 8
    for (label, field) in fields {
        stack.addArrangedSubview(NSTextField(labelWithString: label))
        stack.addArrangedSubview(field)
        field.setAccessibilityLabel(label)
        field.widthAnchor.constraint(equalToConstant: 520).isActive = true
    }
    return stack
}

func attach(_ stack: NSStackView, to alert: NSAlert) {
    stack.layoutSubtreeIfNeeded()
    stack.setFrameSize(stack.fittingSize)
    alert.accessoryView = stack
}

func generateMnemonic() throws -> String {
    var entropy = [UInt8](repeating: 0, count: 16)
    defer { wipe(&entropy) }
    let status = entropy.withUnsafeMutableBytes { bytes in
        SecRandomCopyBytes(kSecRandomDefault, bytes.count, bytes.baseAddress!)
    }
    guard status == errSecSuccess else { throw Failure.derivation }
    return try mnemonicFromEntropy(entropy)
}

func showImport(_ selection: Selection, creating: Bool = false) throws -> StoredWallet {
    let alert = NSAlert()
    alert.messageText = creating ? "Create a new wallet" : "Import an existing wallet"
    alert.informativeText = "Wallet: \(selection.name)\nCurve: \(selection.curve.rawValue)\nPath: \(selection.path)\n" +
        (creating ? "A new 12-word English recovery phrase will be generated on this Mac." : "Expected \(selection.curve == .secp256k1 ? "Ethereum address" : "Ed25519 public key"): \(selection.expectedIdentifier)\nEnter your recovery phrase manually.") +
        "\nThe optional BIP-39 passphrase is NOT your Mac password or a MetaMask unlock password. Secret-entry fields on this screen do not use the clipboard."
    alert.addButton(withTitle: creating ? "Continue to backup" : "Verify and save")
    alert.addButton(withTitle: "Cancel")
    let mnemonic = SecretField(frame: NSRect(x: 0, y: 0, width: 520, height: 26))
    let passphrase = SecretField(frame: mnemonic.frame)
    let confirmation = SecretField(frame: mnemonic.frame)
    var fields: [(String, NSTextField)] = []
    if !creating { fields.append(("English secret recovery phrase (space-separated)", mnemonic)) }
    fields += [("BIP-39 passphrase (optional; spaces are preserved)", passphrase),
               ("Confirm BIP-39 passphrase", confirmation)]
    let stack = form(fields)
    let empty = NSButton(checkboxWithTitle: "I use no BIP-39 passphrase", target: nil, action: nil)
    stack.addArrangedSubview(empty)
    attach(stack, to: alert)
    alert.window.initialFirstResponder = creating ? passphrase : mnemonic
    defer { mnemonic.stringValue = ""; passphrase.stringValue = ""; confirmation.stringValue = "" }
    while true {
        NSApp.activate(ignoringOtherApps: true)
        guard alert.runModal() == .alertFirstButtonReturn else { throw Failure.cancelled }
        do {
            if empty.state == .on {
                guard passphrase.stringValue.isEmpty, confirmation.stringValue.isEmpty else { throw Failure.input }
            } else {
                guard !passphrase.stringValue.isEmpty, passphrase.stringValue == confirmation.stringValue else { throw Failure.input }
            }
            let words = creating ? try generateMnemonic() : normalizedMnemonic(mnemonic.stringValue)
            let wallet = StoredWallet(version: 1, mnemonic: words, passphrase: passphrase.stringValue)
            if !creating {
                var account = try derive(wallet, selection)
                wipe(&account.key)
            }
            return wallet
        } catch { showMessage("Check your wallet details", inputMessage(error)) }
    }
}

func confirmBackup(_ wallet: StoredWallet) throws {
    let reveal = NSAlert()
    reveal.messageText = "Secure your wallet"
    reveal.informativeText = "Your recovery phrase controls your wallet. Write it down in order and keep it offline. Anyone with it can access your funds. This app cannot recover it for you. Make sure no one can see your screen."
    reveal.addButton(withTitle: "Reveal recovery phrase")
    reveal.addButton(withTitle: "Cancel")
    guard reveal.runModal() == .alertFirstButtonReturn else { throw Failure.cancelled }

    let backup = NSAlert()
    backup.messageText = "Back up your secret recovery phrase"
    backup.informativeText = "Write all 12 words down in order. You will re-enter them on the next screen. No wallet has been saved yet." +
        (wallet.passphrase.isEmpty ? "" : "\nAlso keep a separate record of your BIP-39 passphrase; the words alone restore a different wallet.")
    backup.addButton(withTitle: "I wrote it down")
    backup.addButton(withTitle: "Cancel")
    let words = wallet.mnemonic.split(separator: " ")
    let stack = NSStackView()
    stack.orientation = .vertical
    stack.alignment = .leading
    stack.spacing = 6
    var labels: [NSTextField] = []
    for row in 0..<4 {
        let line = NSStackView()
        line.orientation = .horizontal
        line.spacing = 12
        for column in 0..<3 {
            let index = row * 3 + column
            let label = NSTextField(labelWithString: "\(index + 1). \(words[index])")
            label.font = .monospacedSystemFont(ofSize: 15, weight: .medium)
            label.widthAnchor.constraint(equalToConstant: 165).isActive = true
            labels.append(label)
            line.addArrangedSubview(label)
        }
        stack.addArrangedSubview(line)
    }
    let clipboard = RecoveryPhraseClipboard(words: wallet.mnemonic)
    let copy = NSButton(title: "Copy recovery phrase", target: clipboard,
                        action: #selector(RecoveryPhraseClipboard.copyPhrase(_:)))
    stack.addArrangedSubview(copy)
    let notice = NSTextField(wrappingLabelWithString: "Other apps may read the clipboard. It clears after 60 seconds while this app stays open, unless you copy something else.")
    notice.widthAnchor.constraint(equalToConstant: 520).isActive = true
    stack.addArrangedSubview(notice)
    attach(stack, to: backup)
    let response = withExtendedLifetime(clipboard) { backup.runModal() }
    labels.forEach { $0.stringValue = "" }
    guard response == .alertFirstButtonReturn else { throw Failure.cancelled }

    let confirm = NSAlert()
    confirm.messageText = "Confirm your recovery phrase"
    confirm.informativeText = "Type all 12 words from your offline backup, in order. The wallet is saved only after this matches."
    confirm.addButton(withTitle: "Confirm and save")
    confirm.addButton(withTitle: "Cancel")
    let entry = SecretField(frame: NSRect(x: 0, y: 0, width: 520, height: 26))
    entry.allowsPaste = true
    let confirmationForm = form([("Secret recovery phrase", entry)])
    confirmationForm.addArrangedSubview(NSButton(title: "Paste recovery phrase", target: entry,
        action: #selector(SecretField.pasteRecoveryPhrase(_:))))
    attach(confirmationForm, to: confirm)
    confirm.window.initialFirstResponder = entry
    defer { entry.stringValue = "" }
    while true {
        guard confirm.runModal() == .alertFirstButtonReturn else { throw Failure.cancelled }
        if normalizedMnemonic(entry.stringValue) == wallet.mnemonic { return }
        showMessage("Recovery phrase does not match", "Check your offline backup and enter all 12 words in the original order. Nothing has been saved.")
    }
}

func showSetup(_ vault: Vault) throws {
    while true {
        let welcome = NSAlert()
        welcome.messageText = "Welcome to Keychain Wallet"
        welcome.informativeText = "Create or import a BIP-39 recovery phrase and derive secp256k1 or Ed25519 keys. Recovery phrases stay on this Mac in Keychain, protected by macOS authentication. Keep an independent offline backup."
        welcome.addButton(withTitle: "Create a new wallet")
        welcome.addButton(withTitle: "Import an existing wallet")
        welcome.addButton(withTitle: "Quit")
        NSApp.activate(ignoringOtherApps: true)
        let choice = welcome.runModal()
        guard choice != .alertThirdButtonReturn else { return }
        let creating = choice == .alertFirstButtonReturn
        do {
            let algorithm = NSAlert()
            algorithm.messageText = "Choose key algorithm"
            algorithm.informativeText = "The recovery phrase can derive keys for either algorithm. This choice controls the account shown and verified during setup."
            algorithm.addButton(withTitle: "secp256k1 (Ethereum)")
            algorithm.addButton(withTitle: "Ed25519 (SLIP-0010)")
            algorithm.addButton(withTitle: "Back")
            let algorithmChoice = algorithm.runModal()
            guard algorithmChoice != .alertThirdButtonReturn else { throw Failure.cancelled }
            let curve: Curve = algorithmChoice == .alertFirstButtonReturn ? .secp256k1 : .ed25519
            let details = NSAlert()
            details.messageText = creating ? "Name your new wallet" : "Choose the wallet to import"
            details.informativeText = curve == .secp256k1
                ? (creating ? "Choose a local name. The default path selects the first Ethereum account." : "Use an address from your existing wallet or trusted backup to verify the restored account.")
                : "Enter the exact SLIP-0010 derivation path used by your wallet. Every level must end in an apostrophe (hardened). For imports, verify the 32-byte public key as 0x-prefixed hex, not a chain address."
            details.addButton(withTitle: "Continue")
            details.addButton(withTitle: "Back")
            let name = NSTextField(string: "")
            name.placeholderString = "e.g. personal-wallet"
            let path = NSTextField(string: curve == .secp256k1 ? "m/44'/60'/0'/0/0" : "")
            if curve == .ed25519 { path.placeholderString = "e.g. m/44'/501'/0'/0' — check your wallet" }
            let address = NSTextField(string: "")
            address.placeholderString = "0x…"
            var fields = [("Wallet name (letters, numbers, dot, underscore, hyphen)", name),
                          ("Derivation path (advanced)", path)]
            if !creating { fields.append((curve == .secp256k1 ? "Expected Ethereum address" : "Expected Ed25519 public key (0x hex, 32 bytes)", address)) }
            attach(form(fields), to: details)
            details.window.initialFirstResponder = name
            let selection: Selection
            while true {
                guard details.runModal() == .alertFirstButtonReturn else { throw Failure.cancelled }
                let candidate = Selection(name: name.stringValue, path: path.stringValue,
                    expectedIdentifier: creating ? "0x" + String(repeating: "0", count: curve == .secp256k1 ? 40 : 64) : address.stringValue, curve: curve)
                do { try candidate.validate(); selection = candidate; break }
                catch { showMessage("Check your wallet details", inputMessage(error)) }
            }
            let wallet = try showImport(selection, creating: creating)
            var account = creating ? try deriveWallet(wallet, path: selection.path, curve: selection.curve) : try derive(wallet, selection)
            let publicIdentifier = account.identifier
            let publicKey = account.publicKeyString
            wipe(&account.key)
            if creating { try confirmBackup(wallet) }
            try vault.insert(selection.name, wallet)
            var metadata = ["name": selection.name, "path": selection.path, "curve": curve.rawValue, "publicKey": publicKey]
            let addressDetails = curve == .secp256k1 ? "\nAddress: \(publicIdentifier)" : ""
            if curve == .secp256k1 { metadata["address"] = publicIdentifier }
            try emit(metadata)
            showMessage(creating ? "Wallet created" : "Wallet imported",
                "Saved in Keychain.\n\nWallet: \(selection.name)\nCurve: \(curve.rawValue)\nPublic key: \(publicKey)\(addressDetails)\nPath: \(selection.path)\n\nUse these public details in your application. Reading the wallet requires macOS authentication.")
        } catch Failure.cancelled { continue }
        catch { showMessage("Wallet was not saved", inputMessage(error)) }
    }
}

func makeContext(_ reason: String) -> LAContext {
    let context = LAContext()
    context.touchIDAuthenticationAllowableReuseDuration = 0
    context.localizedReason = reason
    context.localizedCancelTitle = "取消"
    return context
}

func emit(_ value: [String: String]) throws {
    let data = try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys])
    FileHandle.standardOutput.write(data)
    FileHandle.standardOutput.write(Data([10]))
}

func pipeDescriptor(_ fd: Int32) -> Bool {
    var stat = stat()
    guard fstat(fd, &stat) == 0 else { return false }
    // Node/libuv may use a socketpair for an anonymous child stdio pipe.
    return (stat.st_mode & S_IFMT) == S_IFIFO || (stat.st_mode & S_IFMT) == S_IFSOCK
}

func writePrivateKey(_ key: [UInt8]) throws {
    try key.withUnsafeBytes { raw in
        var offset = 0
        while offset < raw.count {
            let count = Darwin.write(3, raw.baseAddress!.advanced(by: offset), raw.count - offset)
            if count < 0 && errno == EINTR { continue }
            guard count > 0 else { throw Failure.transport }
            offset += count
        }
    }
}

func main() throws {
    var limit = rlimit(rlim_cur: 0, rlim_max: 0)
    guard setrlimit(RLIMIT_CORE, &limit) == 0 else { throw Failure.configuration }
    signal(SIGPIPE, SIG_IGN)
    let args = Array(CommandLine.arguments.dropFirst())
    let vault = try Vault()
    if args == ["check-signing"] {
        try emit(["status": "SIGNING_CONFIGURATION_OK", "accessGroup": vault.group])
        return
    }
    _ = WalletApplication.shared
    NSApp.setActivationPolicy(args.isEmpty ? .regular : .accessory)
    NSApp.finishLaunching()
    if args.isEmpty { try showSetup(vault); return }
    if args == ["list"] {
        for name in try vault.names() { try emit(["name": name]) }
        return
    }
    if args.count == 2, args[0] == "delete" {
        let name = args[1]
        try Selection(name: name, path: "m/44'/60'/0'/0/0", expectedIdentifier: "0x" + String(repeating: "0", count: 40)).validate()
        let alert = NSAlert()
        alert.messageText = "永久删除钥匙串钱包：\(name)？"
        alert.informativeText = "仅在你已有独立离线备份时继续。删除后无法通过这个工具恢复。"
        alert.addButton(withTitle: "取消")
        alert.addButton(withTitle: "认证并删除")
        NSApp.activate(ignoringOtherApps: true)
        guard alert.runModal() == .alertSecondButtonReturn else { throw Failure.cancelled }
        let context = makeContext("删除钱包 \(name)")
        defer { context.invalidate() }
        _ = try vault.read(name, context: context)
        try vault.delete(name, context: context)
        try emit(["name": name])
        return
    }
    if args == ["--bridge"] {
        guard pipeDescriptor(STDIN_FILENO), pipeDescriptor(3) else { throw Failure.transport }
        var data = Data()
        while true {
            let chunk = FileHandle.standardInput.readData(ofLength: 2049 - data.count)
            if chunk.isEmpty { break }
            data.append(chunk)
            guard data.count <= 2048 else { throw Failure.input }
        }
        guard let request = try? JSONDecoder().decode(KeyRequest.self, from: data) else { throw Failure.input }
        try request.validate()
        let context = makeContext("Wallet: \(request.name)\nCurve: \(request.curve.rawValue)\nPath: \(request.path)")
        defer { context.invalidate() }
        let wallet = try vault.read(request.name, context: context)
        var pair = try deriveWallet(wallet, path: request.path, curve: request.curve)
        defer { wipe(&pair.key) }
        try writePrivateKey(pair.key)
        try emit(["name": request.name, "path": request.path, "curve": request.curve.rawValue, "publicKey": pair.publicKeyString])
        return
    }
    let selection: Selection
    do {
        guard args.count == 4, ["import", "address"].contains(args[0]) else { throw Failure.input }
        selection = Selection(name: args[1], path: args[2], expectedIdentifier: args[3])
    }
    try selection.validate()
    if args[0] == "import" {
        let wallet = try showImport(selection)
        var account = try derive(wallet, selection)
        defer { wipe(&account.key) }
        try vault.insert(selection.name, wallet)
        try emit(["name": selection.name, "path": selection.path, "address": account.identifier])
    } else {
        let reason = "钱包 \(selection.name)\n\(selection.path)\n\(selection.expectedIdentifier)"
        let context = makeContext(reason)
        defer { context.invalidate() }
        let wallet = try vault.read(selection.name, context: context)
        var account = try derive(wallet, selection)
        defer { wipe(&account.key) }
        try emit(["name": selection.name, "path": selection.path, "address": account.identifier])
    }
}

do {
    try main()
} catch {
    let code = (error as? Failure) ?? .input
    // Never interpolate a framework error, input string, mnemonic, or secret.
    FileHandle.standardError.write(Data((code.rawValue + "\n").utf8))
    exit(1)
}
