use bip32::{DerivationPath, XPrv};
use bip39::{Language, Mnemonic};
use derivation_path::DerivationPath as EdPath;
use sha3::{Digest, Keccak256};
use solana_ed25519::ed_sigs::ExtendedSigningKey;
use zeroize::Zeroizing;

fn derive_seed(seed: &[u8], curve: u32, path: &str) -> Result<(Zeroizing<[u8; 32]>, Vec<u8>), i32> {
    match curve {
        0 => {
            let path: DerivationPath = path.parse().map_err(|_| 2)?;
            let child = XPrv::derive_from_path(seed, &path).map_err(|_| 3)?;
            Ok((
                Zeroizing::new(child.to_bytes()),
                child
                    .private_key()
                    .verifying_key()
                    .to_sec1_point(false)
                    .as_bytes()
                    .to_vec(),
            ))
        }
        1 => {
            let path: EdPath = path.parse().map_err(|_| 2)?;
            // This backend implements SLIP-0010 and rejects non-hardened children.
            let child = ExtendedSigningKey::from_seed(seed)
                .derive_path(path.into_iter().map(|index| index.to_bits()))
                .map_err(|_| 3)?;
            Ok((
                Zeroizing::new(child.signing_key().to_secret_key_bytes()),
                child.signing_key().verification_key().as_ref().to_vec(),
            ))
        }
        _ => Err(4),
    }
}

pub fn derive(
    mnemonic: &str,
    passphrase: &str,
    curve: u32,
    path: &str,
) -> Result<(Zeroizing<[u8; 32]>, Vec<u8>), i32> {
    let mnemonic = Mnemonic::parse_in(Language::English, mnemonic).map_err(|_| 1)?;
    let seed = Zeroizing::new(mnemonic.to_seed(passphrase));
    derive_seed(&seed[..], curve, path)
}

pub fn ethereum_address(public: &[u8]) -> [u8; 20] {
    let hash = Keccak256::digest(&public[1..]);
    let mut address = [0; 20];
    address.copy_from_slice(&hash[12..]);
    address
}

pub fn compressed_public_key(public: &[u8]) -> Result<Vec<u8>, i32> {
    bip32::secp256k1::ecdsa::VerifyingKey::from_sec1_bytes(public)
        .map(|key| key.to_sec1_point(true).as_bytes().to_vec())
        .map_err(|_| 4)
}

#[cfg(test)]
mod tests {
    use super::*;
    const WORDS: &str = "test test test test test test test test test test test junk";
    const PATH: &str = "m/44'/60'/0'/0/0";

    fn bytes(hex: &str) -> Vec<u8> {
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn slip10_official_ed25519_vector() {
        // SLIP-0010 test vector 1: master and final hardened child.
        let seed = bytes("000102030405060708090a0b0c0d0e0f");
        for (path, secret, public) in [
            (
                "m",
                "2b4be7f19ee27bbf30c667b642d5f4aa69fd169872f8fc3059c08ebae2eb19e7",
                "a4b2856bfec510abab89753fac1ac0e1112364e7d250545963f135f2a33188ed",
            ),
            (
                "m/0'/1'/2'/2'/1000000000'",
                "8f94d394a8e8fd6b1bc2f3f49f5c47e385281d5c17e65324b0f62483e37e8793",
                "3c24da049451555d51a7014a37337aa4e12d41e485abccfa46b47dfb2af54b7a",
            ),
        ] {
            let (key, result) = derive_seed(&seed, 1, path).unwrap();
            assert_eq!(key.as_slice(), bytes(secret));
            assert_eq!(result, bytes(public));
        }
        assert!(derive_seed(&seed, 1, "m/0'/1").is_err());
        assert!(derive_seed(&seed, 1, "m/2147483648'").is_err());
        assert!(derive_seed(&seed, 1, "bad/path").is_err());
        assert!(derive_seed(&seed, 2, "m/0'").is_err());
    }

    #[test]
    fn ed25519_extended_keys_clear_secret_and_chain_code() {
        use zeroize::Zeroize;
        assert!(std::mem::needs_drop::<ExtendedSigningKey>());
        let mut key = ExtendedSigningKey::from_seed(&[1; 32]);
        key.zeroize();
        assert_eq!(key.signing_key().as_secret_key_bytes(), &[0; 32]);
        assert_eq!(key.chain_code(), &[0; 32]);
    }

    #[test]
    fn solana_derivation_matches_previous_backend() {
        // Captured from ed25519-dalek-bip32 using the public Hardhat mnemonic.
        for (passphrase, path, secret, public) in [
            (
                "",
                "m/44'/501'/0'/0'",
                "a01c67efb5cb8f62f15907a9fcf124c73d2695c8a9a31f21c05742a31ca1dc72",
                "0bf32b9f0db09672038fea36139b18f98a5f0149ef4ce0332e44b9a77e83c22d",
            ),
            (
                "",
                "m/44'/501'/1'/0'",
                "3f25626cf245c8dd65a2b91a70e6f4c285182489fda230e2553c3752aa962b61",
                "9245441f4a752ea6328c6d8d2d2afcb36d131db998a47ca04d6d5d8587d0b012",
            ),
            (
                "",
                "m/2147483647'",
                "ec130d14b2b636887c6c9b91afc937d0e84fe4fde149e2e9766aa926f9830c63",
                "1946234b8b7c825e1d9f38251af3cd6edd8dfd80f8b41bff84a48dbe28a1348e",
            ),
            (
                "TREZOR",
                "m/44'/501'/0'/0'",
                "81323bfedeabf1da2af3fc3d5370be76862f3e4a0e674efb0e3ddc81c1b6ed5f",
                "0bd89cf16f4f7a40a2839b8ca08832cebec6737bd30cae4af528e59e6c6b0f33",
            ),
        ] {
            let (key, result) = derive(WORDS, passphrase, 1, path).unwrap();
            assert_eq!(key.as_slice(), bytes(secret));
            assert_eq!(result, bytes(public));
        }
    }

    #[test]
    fn entropy_to_mnemonic_bip39_vector() {
        let entropy = [0u8; 16];
        let words = mnemonic_from_entropy(&entropy).unwrap();
        assert_eq!(words.as_str(), "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about");
        assert_eq!(
            Mnemonic::parse_in(Language::English, words.as_str())
                .unwrap()
                .to_entropy(),
            entropy
        );
    }

    #[test]
    fn public_hardhat_vector() {
        let (_, public) = derive(WORDS, "", 0, PATH).unwrap();
        let address = ethereum_address(&public);
        assert_eq!(
            address,
            [
                0xf3, 0x9f, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xf6, 0xf4, 0xce, 0x6a, 0xb8, 0x82, 0x72,
                0x79, 0xcf, 0xff, 0xb9, 0x22, 0x66
            ]
        );
        let compressed = compressed_public_key(&public).unwrap();
        assert_eq!(compressed.len(), 33);
        assert!(matches!(compressed[0], 2 | 3));
        let restored = bip32::secp256k1::ecdsa::VerifyingKey::from_sec1_bytes(&compressed)
            .unwrap()
            .to_sec1_point(false);
        assert_eq!(restored.as_bytes(), public.as_slice());
        assert_eq!(compressed_public_key(&compressed).unwrap(), compressed);
        assert!(compressed_public_key(&[0; 33]).is_err());
    }

    #[test]
    fn bip39_passphrase_vector_and_nfkd() {
        let words = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let mnemonic = Mnemonic::parse_in(Language::English, words).unwrap();
        let seed = Zeroizing::new(mnemonic.to_seed("TREZOR"));
        let hex: String = seed.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(hex, "c55257c360c07c72029aebc1b53c05ed0362ada38ead3e3e9efa3708e53495531f09a6987599d18264c1e1c92f2cf141630c7a3c4ab7c81b2f001698e7463b04");
        assert_eq!(
            *derive(WORDS, "é", 0, PATH).unwrap().0,
            *derive(WORDS, "e\u{301}", 0, PATH).unwrap().0
        );
        assert_ne!(
            *derive(WORDS, "", 0, PATH).unwrap().0,
            *derive(WORDS, " ", 0, PATH).unwrap().0
        );
    }

    #[test]
    fn rejects_bad_checksum_and_path() {
        assert!(derive("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon", "", 0, PATH).is_err());
        assert!(derive(WORDS, "", 0, "bad/path").is_err());
    }
}

/// Convert OS-supplied entropy to an English BIP-39 phrase.
pub fn mnemonic_from_entropy(entropy: &[u8; 16]) -> Result<Zeroizing<String>, i32> {
    Mnemonic::from_entropy_in(Language::English, entropy)
        .map(|m| Zeroizing::new(m.to_string()))
        .map_err(|_| 1)
}
