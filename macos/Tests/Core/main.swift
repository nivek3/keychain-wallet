import Foundation

// Public Hardhat vector and independent ethers v6 derivation references. Never fund.
let words = "test test test test test test test test test test test junk"
let path = "m/44'/60'/0'/0/0"
let address = "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266"

func selection(_ expected: String = address, _ derivationPath: String = path) -> Selection {
    Selection(name: "public-test", path: derivationPath, expectedIdentifier: expected)
}

func requireFailure(_ expected: Failure, _ body: () throws -> Void) {
    do {
        try body()
        fatalError("Expected rejection")
    } catch {
        precondition((error as? Failure) == expected, "Unexpected failure code")
    }
}

do {
    let generated = try mnemonicFromEntropy([UInt8](repeating: 0, count: 16))
    precondition(generated == "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
    var generatedAccount = try deriveWallet(StoredWallet(version: 1, mnemonic: generated, passphrase: ""), path: path)
    precondition(generatedAccount.identifier == "0x9858effd232b4033e47d90003d41ec34ecaeda94")
    wipe(&generatedAccount.key)
    requireFailure(.input) { _ = try mnemonicFromEntropy([0]) }
    requireFailure(.input) {
        _ = try deriveWallet(StoredWallet(version: 1, mnemonic: generated, passphrase: ""), path: "m/0/0")
    }
    precondition(normalizedMnemonic("  test\n test\t junk  ") == "test test junk")
    // Independent Node HMAC-SHA512 / OpenSSL references for this public mnemonic.
    let edPath = "m/44'/501'/0'/0'"
    for (passphrase, path, publicKey) in [
        ("", edPath, "0bf32b9f0db09672038fea36139b18f98a5f0149ef4ce0332e44b9a77e83c22d"),
        ("", "m/44'/501'/1'/0'", "9245441f4a752ea6328c6d8d2d2afcb36d131db998a47ca04d6d5d8587d0b012"),
        ("TREZOR", edPath, "0bd89cf16f4f7a40a2839b8ca08832cebec6737bd30cae4af528e59e6c6b0f33"),
        ("é", edPath, "e0d2365114a62d771831bed83e660ef8d88c0b35bf9a166bc98e1f32b8255254"),
        ("e\u{301}", edPath, "e0d2365114a62d771831bed83e660ef8d88c0b35bf9a166bc98e1f32b8255254"),
    ] {
        let request = Selection(name: "public-ed", path: path, expectedIdentifier: "0x" + publicKey, curve: .ed25519)
        let wallet = StoredWallet(version: 1, mnemonic: words, passphrase: passphrase)
        var pair = try derive(wallet, request)
        precondition(pair.key.count == 32 && pair.publicKey.count == 32)
        precondition(pair.publicKeyString == "0x" + publicKey)
        wipe(&pair.key)
    }
    for path in ["m", "m/0'/1", "m/2147483648'", "m/0'\n"] {
        requireFailure(.input) { try KeyRequest(name: "public-ed", path: path, curve: .ed25519).validate() }
    }
    for json in [#"{"name":"public-ed","path":"m/0'"}"#, #"{"name":"public-ed","path":"m/0'","curve":"p256"}"#] {
        precondition((try? JSONDecoder().decode(KeyRequest.self, from: Data(json.utf8))) == nil)
    }
    requireFailure(.publicKey) {
        _ = try derive(StoredWallet(version: 1, mnemonic: words, passphrase: ""),
            Selection(name: "public-ed", path: edPath, expectedIdentifier: "0x" + String(repeating: "0", count: 64), curve: .ed25519))
    }
    try selection().validate()
    var account = try derive(StoredWallet(version: 1, mnemonic: words, passphrase: ""), selection())
    precondition(account.identifier == address)
    precondition(account.key.count == 32 && account.key.contains { $0 != 0 })
    wipe(&account.key)
    precondition(account.key.allSatisfy { $0 == 0 })

    for (passphrase, expected, derivationPath) in [
        ("TREZOR", "0x9313778b3753108128b9c476ebdd42fbd566f4ed", path),
        ("é", "0xb1f6732595f1cdf8fb042c010fd2dffc5f900f75", path),
        ("e\u{301}", "0xb1f6732595f1cdf8fb042c010fd2dffc5f900f75", path),
        ("", "0x441b4a730bdea051a4e14847e3bcd588a29a1315", "m/44'/60'/1'/1/5"),
    ] {
        let request = selection(expected, derivationPath)
        try request.validate()
        var derived = try derive(StoredWallet(version: 1, mnemonic: words, passphrase: passphrase), request)
        precondition(derived.identifier == expected)
        wipe(&derived.key)
    }
    for request in [
        Selection(name: "public-test\n", path: path, expectedIdentifier: address),
        selection(address + "\n"), selection(address, path + "\n"),
        selection(address, "m/44'/60'/2147483648'/0/0"),
    ] {
        requireFailure(.input) { try request.validate() }
    }
    requireFailure(.address) {
        _ = try derive(StoredWallet(version: 1, mnemonic: words, passphrase: " "), selection())
    }
    requireFailure(.mnemonic) {
        _ = try derive(StoredWallet(version: 1, mnemonic: "invalid public words", passphrase: ""), selection())
    }
    print("Swift/Rust public-vector boundary checks passed")
} catch {
    fatalError("Unexpected native test failure")
}
