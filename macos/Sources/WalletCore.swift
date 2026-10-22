import Foundation

enum Failure: String, Error {
    case cancelled = "CANCELLED", authentication = "AUTHENTICATION_FAILED"
    case missing = "NOT_FOUND", duplicate = "ALREADY_EXISTS", keychain = "KEYCHAIN_ERROR"
    case configuration = "SIGNING_CONFIGURATION", input = "INVALID_INPUT"
    case mnemonic = "INVALID_MNEMONIC", derivation = "DERIVATION_FAILED"
    case publicKey = "PUBLIC_KEY_MISMATCH"
    case address = "ADDRESS_MISMATCH", transport = "TRANSPORT_ERROR", corrupt = "INVALID_ITEM"
}

enum Curve: String, Codable {
    case secp256k1, ed25519
    var cryptoID: UInt32 { self == .secp256k1 ? 0 : 1 }
}

struct KeyRequest: Codable {
    let name: String
    let path: String
    let curve: Curve

    func validate() throws {
        let pattern = curve == .secp256k1 ? #"\Am/44'/60'/[0-9]+'/[01]/[0-9]+\z"# : #"\Am(/[0-9]+')+\z"#
        guard name.range(of: #"\A[A-Za-z0-9][A-Za-z0-9._-]{0,63}\z"#, options: .regularExpression) != nil,
              path.utf8.count <= 128,
              path.range(of: pattern, options: .regularExpression) != nil else { throw Failure.input }
        for component in path.split(separator: "/").dropFirst() {
            guard let value = UInt32(component.replacingOccurrences(of: "'", with: "")),
                  value < 0x80000000 else { throw Failure.input }
        }
    }
}

struct Selection {
    let name: String
    let path: String
    let expectedIdentifier: String
    var curve: Curve = .secp256k1

    func validate() throws {
        try KeyRequest(name: name, path: path, curve: curve).validate()
        let pattern = curve == .secp256k1 ? #"\A0x[0-9a-fA-F]{40}\z"# : #"\A0x[0-9a-fA-F]{64}\z"#
        guard expectedIdentifier.range(of: pattern, options: .regularExpression) != nil else { throw Failure.input }
    }
}

struct StoredWallet: Codable {
    let version: Int
    let mnemonic: String
    let passphrase: String
}

func wipe(_ bytes: inout [UInt8]) {
    bytes.withUnsafeMutableBufferPointer { raw in
        if let base = raw.baseAddress { mw_zeroize(base, raw.count) }
    }
}

struct Derived {
    var key = [UInt8](repeating: 0, count: 32)
    var publicKey: [UInt8]
    let identifier: String
    var publicKeyString: String { hex(publicKey) }
}

func hex(_ bytes: [UInt8]) -> String { "0x" + bytes.map { String(format: "%02x", $0) }.joined() }

func normalizedMnemonic(_ text: String) -> String {
    text.split(whereSeparator: { $0.isWhitespace }).joined(separator: " ")
}

func mnemonicFromEntropy(_ entropy: [UInt8]) throws -> String {
    guard entropy.count == 16 else { throw Failure.input }
    var output = [UInt8](repeating: 0, count: 256)
    defer { wipe(&output) }
    let status = entropy.withUnsafeBufferPointer { bytes in
        output.withUnsafeMutableBufferPointer { result in
            mw_mnemonic_from_entropy(bytes.baseAddress!, result.baseAddress!)
        }
    }
    guard status == 0, let end = output.firstIndex(of: 0), end > 0,
          let words = String(bytes: output[..<end], encoding: .utf8) else { throw Failure.derivation }
    return words
}

func deriveWallet(_ wallet: StoredWallet, path derivationPath: String, curve: Curve = .secp256k1) throws -> Derived {
    try KeyRequest(name: "validation", path: derivationPath, curve: curve).validate()
    var mnemonic = Array(wallet.mnemonic.utf8)
    var passphrase = Array(wallet.passphrase.utf8)
    let path = Array(derivationPath.utf8)
    defer { wipe(&mnemonic); wipe(&passphrase) }
    guard !mnemonic.isEmpty, mnemonic.count <= 4096, passphrase.count <= 4096 else { throw Failure.input }
    let mnemonicLength = mnemonic.count
    let passphraseLength = passphrase.count
    // A non-null buffer is also required for the empty BIP-39 passphrase.
    passphrase.append(0)
    var keyBytes = [UInt8](repeating: 0, count: 32)
    var publicBytes = [UInt8](repeating: 0, count: 65)
    let status = mnemonic.withUnsafeBufferPointer { m in
        passphrase.withUnsafeBufferPointer { p in
            path.withUnsafeBufferPointer { d in
                keyBytes.withUnsafeMutableBufferPointer { key in
                    publicBytes.withUnsafeMutableBufferPointer { publicKey in
                        mw_derive(curve.cryptoID, m.baseAddress!, mnemonicLength, p.baseAddress!, passphraseLength,
                                  d.baseAddress!, d.count, key.baseAddress!, publicKey.baseAddress!)
                    }
                }
            }
        }
    }
    guard status == 0 else {
        wipe(&keyBytes)
        throw status == 1 ? Failure.mnemonic : Failure.derivation
    }
    if curve == .ed25519 { publicBytes = Array(publicBytes.prefix(32)) }
    var identifier = hex(publicBytes)
    if curve == .secp256k1 {
        var address = [UInt8](repeating: 0, count: 20)
        let result = publicBytes.withUnsafeBufferPointer { publicKey in
            address.withUnsafeMutableBufferPointer { output in
                mw_ethereum_address(publicKey.baseAddress!, output.baseAddress!)
            }
        }
        guard result == 0 else { wipe(&keyBytes); throw Failure.derivation }
        identifier = hex(address)
    }
    return Derived(key: keyBytes, publicKey: publicBytes, identifier: identifier)
}

func derive(_ wallet: StoredWallet, _ selection: Selection) throws -> Derived {
    try selection.validate()
    var account = try deriveWallet(wallet, path: selection.path, curve: selection.curve)
    guard account.identifier.lowercased() == selection.expectedIdentifier.lowercased() else {
        wipe(&account.key)
        throw selection.curve == .secp256k1 ? Failure.address : Failure.publicKey
    }
    return account
}
